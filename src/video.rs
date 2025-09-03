use crate::Error;
use glib::FlagsClass;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_app::prelude::*;
use iced::widget::image as img;
use std::num::NonZeroU8;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use subtitles::SubtitleFontDescription;

/// Position in the media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Position based on time.
    ///
    /// Not the most accurate format for videos.
    Time(Duration),
    /// Position based on nth frame.
    Frame(u64),
}

impl From<Position> for gst::GenericFormattedValue {
    fn from(pos: Position) -> Self {
        match pos {
            Position::Time(t) => gst::ClockTime::from_nseconds(t.as_nanos() as _).into(),
            Position::Frame(f) => gst::format::Default::from_u64(f).into(),
        }
    }
}

impl From<Duration> for Position {
    fn from(t: Duration) -> Self {
        Position::Time(t)
    }
}

impl From<u64> for Position {
    fn from(f: u64) -> Self {
        Position::Frame(f)
    }
}

#[derive(Debug)]
pub(crate) struct Frame(gst::Sample);

impl Frame {
    pub fn empty() -> Self {
        Self(gst::Sample::builder().build())
    }

    pub fn readable(&self) -> Option<gst::BufferMap<'_, gst::buffer::Readable>> {
        self.0.buffer().and_then(|x| x.map_readable().ok())
    }
}

#[derive(Debug)]
/// Video filters applied to the GStreamer pipeline. For `playbin` this mirrors
/// the `video-filter` property.Only `videobalance` and `gamma` filters are
/// currently supported.
pub struct VideoFilters {
    balance: Option<gst::Element>,
    gamma: Option<gst::Element>,
}

impl Default for VideoFilters {
    fn default() -> Self {
        VideoFilters::none()
    }
}

impl VideoFilters {
    /// Returns an empty [`VideoFilters`]. No filters are applied to the
    /// playback.
    pub fn none() -> Self {
        Self {
            balance: None,
            gamma: None,
        }
    }

    /// Returns a [`VideoFilters`] with only the balance filter set. The brightness,
    /// saturation, hue and contrast filters can thus be changed.
    pub fn balance(balance: gst::Element) -> Self {
        Self {
            balance: Some(balance),
            ..Default::default()
        }
    }

    /// Returns a [`VideoFilters`] with only the gamma filter set. The gamma
    /// filter can thus be changed.
    pub fn gamma(gamma: gst::Element) -> Self {
        Self {
            gamma: Some(gamma),
            ..Default::default()
        }
    }

    /// Returns a [`VideoFilters`] with both balance and gamma filters set.
    pub fn all(balance: gst::Element, gamma: gst::Element) -> Self {
        Self {
            balance: Some(balance),
            gamma: Some(gamma),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Internal {
    pub(crate) id: u64,

    pub(crate) bus: gst::Bus,
    pub(crate) source: gst::Pipeline,
    pub(crate) video_filters: VideoFilters,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    pub(crate) duration: Duration,
    pub(crate) speed: f64,
    pub(crate) sync_av: bool,

    pub(crate) show_subtitles: bool,
    pub(crate) subtitle_description: SubtitleFontDescription,

    pub(crate) frame: Arc<Mutex<Frame>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) last_frame_time: Arc<Mutex<Instant>>,
    pub(crate) looping: bool,
    pub(crate) is_eos: bool,
    pub(crate) restart_stream: bool,
    pub(crate) sync_av_avg: u64,
    pub(crate) sync_av_counter: u64,
}

impl Internal {
    pub(crate) fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        let position = position.into();

        // gstreamer complains if the start & end value types aren't the same
        match &position {
            Position::Time(_) => self.source.seek(
                self.speed,
                gst::SeekFlags::FLUSH
                    | if accurate {
                        gst::SeekFlags::ACCURATE
                    } else {
                        gst::SeekFlags::empty()
                    },
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::Set,
                gst::ClockTime::NONE,
            )?,
            Position::Frame(_) => self.source.seek(
                self.speed,
                gst::SeekFlags::FLUSH
                    | if accurate {
                        gst::SeekFlags::ACCURATE
                    } else {
                        gst::SeekFlags::empty()
                    },
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::Set,
                gst::format::Default::NONE,
            )?,
        };

        Ok(())
    }

    pub(crate) fn set_speed(&mut self, speed: f64) -> Result<(), Error> {
        let Some(position) = self.source.query_position::<gst::ClockTime>() else {
            return Err(Error::Caps);
        };
        if speed > 0.0 {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                position,
                gst::SeekType::End,
                gst::ClockTime::from_seconds(0),
            )?;
        } else {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                gst::ClockTime::from_seconds(0),
                gst::SeekType::Set,
                position,
            )?;
        }
        self.speed = speed;
        Ok(())
    }

    pub(crate) fn restart_stream(&mut self) -> Result<(), Error> {
        self.is_eos = false;
        self.set_paused(false);
        self.seek(0, false)?;
        Ok(())
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.source
            .set_state(if paused {
                gst::State::Paused
            } else {
                gst::State::Playing
            })
            .unwrap(/* state was changed in ctor; state errors caught there */);

        // Set restart_stream flag to make the stream restart on the next Message::NextFrame
        if self.is_eos && !paused {
            self.restart_stream = true;
        }
    }

    pub(crate) fn paused(&self) -> bool {
        self.source.state(gst::ClockTime::ZERO).1 == gst::State::Paused
    }

    /// Syncs audio with video when there is (inevitably) latency presenting the frame.
    pub(crate) fn set_av_offset(&mut self, offset: Duration) {
        if self.sync_av {
            self.sync_av_counter += 1;
            self.sync_av_avg = self.sync_av_avg * (self.sync_av_counter - 1) / self.sync_av_counter
                + offset.as_nanos() as u64 / self.sync_av_counter;
            if self.sync_av_counter % 128 == 0 {
                self.source
                    .set_property("av-offset", -(self.sync_av_avg as i64));
            }
        }
    }

    fn toggle_subtitles(&mut self) {
        let pipeline = &self.source;

        let flags = pipeline.property_value("flags");
        let flags_class = FlagsClass::with_type(flags.type_()).unwrap();
        let builder = flags_class.builder_with_value(flags).unwrap();

        let flags = if self.show_subtitles {
            builder.unset_by_nick("text")
        } else {
            builder.set_by_nick("text")
        }
        .build()
        .unwrap();

        pipeline.set_property_from_value("flags", &flags);
        self.show_subtitles = !self.show_subtitles;
    }

    fn set_subtitle_description(&mut self, description: SubtitleFontDescription) {
        let pipeline = &self.source;

        self.subtitle_description = description;
        pipeline.set_property("subtitle-font-desc", description.to_string());
    }

    fn set_text(&mut self, text: TextTag) {
        self.source.set_property("current-text", text.id);
    }
}

/// A multimedia video loaded from a URI (e.g., a local file path or HTTP stream).
#[derive(Debug)]
pub struct Video(pub(crate) RwLock<Internal>);

impl Drop for Video {
    fn drop(&mut self) {
        let inner = self.0.get_mut().expect("failed to lock");

        inner
            .source
            .set_state(gst::State::Null)
            .expect("failed to set state");

        inner.alive.store(false, Ordering::SeqCst);
        if let Some(worker) = inner.worker.take() {
            if let Err(err) = worker.join() {
                match err.downcast_ref::<String>() {
                    Some(e) => log::error!("Video thread panicked: {e}"),
                    None => log::error!("Video thread panicked with unknown reason"),
                }
            }
        }
    }
}

impl Video {
    /// Create a new video player from a given video which loads from `uri`.
    /// Both balance and gamma filters are enabled and set to their default
    /// values.
    ///
    /// Note that live sources will report the duration to be zero.
    pub fn new(uri: &url::Url) -> Result<Self, Error> {
        gst::init()?;

        let pipeline = format!("playbin uri=\"{}\"  video-sink=\"videoscale ! videoconvert ! appsink name=iced_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\" video-filter=\"videobalance name=balance ! gamma name=gamma\" audio-filter= \"pitch name=pitch\"", uri.as_str());
        let pipeline = gst::parse::launch(pipeline.as_ref())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Error::Cast)?;

        let video_sink: gst::Element = pipeline.property("video-sink");
        let pad = video_sink.pads().first().cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let video_sink = bin.by_name("iced_video").unwrap();
        let video_sink = video_sink.downcast::<gst_app::AppSink>().unwrap();

        let filter: gst::Element = pipeline.property("video-filter");
        let pad = filter.pads().first().cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let balance = bin.by_name("balance").unwrap();

        let gamma: gst::Element = bin.by_name("gamma").unwrap();

        let filters = VideoFilters::all(balance, gamma);

        let mut output = Self::from_gst_pipeline(
            pipeline,
            video_sink,
            false,
            SubtitleFontDescription::default(),
        )?;
        output.set_video_filters(filters);

        Ok(output)
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink.
    /// Expects an `appsink` plugin with `caps=video/x-raw,format=NV12`.
    ///
    /// **Note:** Many functions of [`Video`] assume a `playbin` pipeline.
    /// Non-`playbin` pipelines given here may not have full functionality.
    pub fn from_gst_pipeline(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        show_subtitles: bool,
        subtitle_description: SubtitleFontDescription,
    ) -> Result<Self, Error> {
        gst::init()?;
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

        let flags = pipeline.property_value("flags");
        let flags_class = FlagsClass::with_type(flags.type_()).unwrap();
        let builder = flags_class.builder_with_value(flags).unwrap();
        let flags = if show_subtitles {
            builder.set_by_nick("text")
        } else {
            builder.unset_by_nick("text")
        }
        .build()
        .unwrap();

        pipeline.set_property_from_value("flags", &flags);

        pipeline.set_property("subtitle-font-desc", subtitle_description.to_string());

        // We need to ensure we stop the pipeline if we hit an error,
        // or else there may be audio left playing in the background.
        macro_rules! cleanup {
            ($expr:expr) => {
                $expr.map_err(|e| {
                    let _ = pipeline.set_state(gst::State::Null);
                    e
                })
            };
        }

        let pad = video_sink.pads().first().cloned().unwrap();

        cleanup!(pipeline.set_state(gst::State::Playing))?;

        // wait for up to 5 seconds until the decoder gets the source capabilities
        cleanup!(pipeline.state(gst::ClockTime::from_seconds(5)).0)?;

        // extract resolution and framerate
        // TODO(jazzfool): maybe we want to extract some other information too?
        let caps = cleanup!(pad.current_caps().ok_or(Error::Caps))?;
        let s = cleanup!(caps.structure(0).ok_or(Error::Caps))?;
        let width = cleanup!(s.get::<i32>("width").map_err(|_| Error::Caps))?;
        let height = cleanup!(s.get::<i32>("height").map_err(|_| Error::Caps))?;
        // resolution should be mod4
        let width = ((width + 4 - 1) / 4) * 4;
        let framerate = cleanup!(s.get::<gst::Fraction>("framerate").map_err(|_| Error::Caps))?;
        let framerate = framerate.numer() as f64 / framerate.denom() as f64;

        if framerate.is_nan()
            || framerate.is_infinite()
            || framerate < 0.0
            || framerate.abs() < f64::EPSILON
        {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(Error::Framerate(framerate));
        }

        let duration = Duration::from_nanos(
            pipeline
                .query_duration::<gst::ClockTime>()
                .map(|duration| duration.nseconds())
                .unwrap_or(0),
        );

        let sync_av = pipeline.has_property("av-offset", None);

        // NV12 = 12bpp
        let frame = Arc::new(Mutex::new(Frame::empty()));
        let upload_frame = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let last_frame_time = Arc::new(Mutex::new(Instant::now()));

        let frame_ref = Arc::clone(&frame);
        let upload_frame_ref = Arc::clone(&upload_frame);
        let alive_ref = Arc::clone(&alive);
        let last_frame_time_ref = Arc::clone(&last_frame_time);

        let pipeline_ref = pipeline.clone();

        let worker = std::thread::spawn(move || {
            while alive_ref.load(Ordering::Acquire) {
                if let Err(gst::FlowError::Error) = (|| -> Result<(), gst::FlowError> {
                    let sample =
                        if pipeline_ref.state(gst::ClockTime::ZERO).1 != gst::State::Playing {
                            video_sink
                                .try_pull_preroll(gst::ClockTime::from_mseconds(16))
                                .ok_or(gst::FlowError::Eos)?
                        } else {
                            video_sink
                                .try_pull_sample(gst::ClockTime::from_mseconds(16))
                                .ok_or(gst::FlowError::Eos)?
                        };

                    *last_frame_time_ref
                        .lock()
                        .map_err(|_| gst::FlowError::Error)? = Instant::now();

                    {
                        let mut frame_guard =
                            frame_ref.lock().map_err(|_| gst::FlowError::Error)?;
                        *frame_guard = Frame(sample);
                    }

                    upload_frame_ref.swap(true, Ordering::SeqCst);

                    Ok(())
                })() {
                    log::error!("error pulling frame");
                }
            }
        });

        Ok(Video(RwLock::new(Internal {
            id,

            bus: pipeline.bus().unwrap(),
            source: pipeline,
            video_filters: VideoFilters::default(),
            alive,
            worker: Some(worker),

            width,
            height,
            framerate,
            duration,
            speed: 1.0,
            sync_av,

            show_subtitles,
            subtitle_description,

            frame,
            upload_frame,
            last_frame_time,
            looping: false,
            is_eos: false,
            restart_stream: false,
            sync_av_avg: 0,
            sync_av_counter: 0,
        })))
    }

    /// Sets the [`VideoFilters`] of the [`Video`].
    pub fn set_video_filters(&mut self, filters: impl Into<VideoFilters>) {
        self.get_mut().video_filters = filters.into();
    }

    /// Sets only the balance filter of the [`Video`].
    pub fn set_video_balance_filter(&mut self, video_balance: gst::Element) {
        self.get_mut().video_filters.balance = Some(video_balance);
    }

    /// Sets only the gamma filter of the [`Video`].
    pub fn set_gamma_filter(&mut self, gamma_bin: gst::Element) {
        self.get_mut().video_filters.gamma = Some(gamma_bin);
    }

    pub(crate) fn read(&self) -> impl Deref<Target = Internal> + '_ {
        self.0.read().expect("lock")
    }

    pub(crate) fn write(&self) -> impl DerefMut<Target = Internal> + '_ {
        self.0.write().expect("lock")
    }

    pub(crate) fn get_mut(&mut self) -> impl DerefMut<Target = Internal> + '_ {
        self.0.get_mut().expect("lock")
    }

    /// Get the size/resolution of the video as `(width, height)`.
    pub fn size(&self) -> (i32, i32) {
        (self.read().width, self.read().height)
    }

    /// Get the framerate of the video as frames per second.
    pub fn framerate(&self) -> f64 {
        self.read().framerate
    }

    /// Returns the gamma level of the playback. The default gamma level is 1.0.
    pub fn gamma(&self) -> f64 {
        let filters = &self.read().video_filters;

        match filters.gamma.as_ref() {
            Some(gamma) => gamma.property("gamma"),
            None => 1.0,
        }
    }

    /// Sets the gamma level of the playback.
    /// The gamma is clamped to the range `[1.0, 3.0]`.
    pub fn set_gamma(&mut self, gamma: f64) {
        let filters = &mut self.get_mut().video_filters;
        let Some(bin) = filters.gamma.as_mut() else {
            return;
        };
        let gamma = gamma.clamp(1.0, 3.0);
        bin.set_property("gamma", gamma);
    }

    /// Returns the brightness of the playback. The default brightness is 0.0.
    pub fn brightness(&self) -> f64 {
        let filters = &self.read().video_filters;

        match filters.balance.as_ref() {
            Some(balance) => balance.property("brightness"),
            None => 0.0,
        }
    }

    /// Sets the brightness of the playback. The brightness is clamped to the
    /// range `[-1.0, 1.0]`.
    pub fn set_brightness(&mut self, brightness: f64) {
        let filters = &mut self.get_mut().video_filters;
        let Some(balance) = filters.balance.as_mut() else {
            return;
        };
        let brightness = brightness.clamp(-1.0, 1.0);
        balance.set_property("brightness", brightness);
    }

    /// Returns the contrast of the playback. The default contrast is 1.0.
    pub fn contrast(&self) -> f64 {
        let filters = &self.read().video_filters;

        match filters.balance.as_ref() {
            Some(balance) => balance.property("contrast"),
            None => 1.0,
        }
    }

    /// Sets the contrast of the playback. The contrast is clamped to the range
    /// `[0.0, 2.0]`.
    pub fn set_contrast(&mut self, contrast: f64) {
        let filters = &mut self.get_mut().video_filters;
        let Some(balance) = filters.balance.as_mut() else {
            return;
        };
        let contrast = contrast.clamp(0.0, 2.0);
        balance.set_property("contrast", contrast);
    }

    /// Returns the hue of the playback. The default hue is 0.0.
    pub fn hue(&self) -> f64 {
        let filters = &self.read().video_filters;

        match filters.balance.as_ref() {
            Some(balance) => balance.property("hue"),
            None => 0.0,
        }
    }

    /// Sets the hue of the playback. The hue is clamped to the range `[-1.0,
    /// 1.0]`.
    pub fn set_hue(&mut self, hue: f64) {
        let filters = &mut self.get_mut().video_filters;
        let Some(balance) = filters.balance.as_mut() else {
            return;
        };
        let hue = hue.clamp(-1.0, 1.0);
        balance.set_property("hue", hue);
    }

    /// Returns the saturation of the playback. The default saturation is 1.0.
    pub fn saturation(&self) -> f64 {
        let filters = &self.read().video_filters;

        match filters.balance.as_ref() {
            Some(balance) => balance.property("saturation"),
            None => 1.0,
        }
    }

    /// Sets the saturation fo the playback. Saturation is clamped to the range
    /// `[0.0, 2.0]`.
    pub fn set_saturation(&mut self, saturation: f64) {
        let filters = &mut self.get_mut().video_filters;
        let Some(balance) = filters.balance.as_mut() else {
            return;
        };
        let saturation = saturation.clamp(0.0, 2.0);
        balance.set_property("saturation", saturation);
    }

    /// Set the volume multiplier of the audio.
    /// `0.0` = 0% volume, `1.0` = 100% volume.
    ///
    /// This uses a linear scale, for example `0.5` is perceived as half as loud.
    pub fn set_volume(&mut self, volume: f64) {
        self.get_mut().source.set_property("volume", volume);
        self.set_muted(self.muted()); // for some reason gstreamer unmutes when changing volume?
    }

    /// Get the volume multiplier of the audio.
    pub fn volume(&self) -> f64 {
        self.read().source.property("volume")
    }

    /// Set if the audio is muted or not, without changing the volume.
    pub fn set_muted(&mut self, muted: bool) {
        self.get_mut().source.set_property("mute", muted);
    }

    /// Get if the audio is muted or not.
    pub fn muted(&self) -> bool {
        self.read().source.property("mute")
    }

    /// Get if the stream ended or not.
    pub fn eos(&self) -> bool {
        self.read().is_eos
    }

    /// Get if the media will loop or not.
    pub fn looping(&self) -> bool {
        self.read().looping
    }

    /// Set if the media will loop or not.
    pub fn set_looping(&mut self, looping: bool) {
        self.get_mut().looping = looping;
    }

    /// Set if the media is paused or not.
    pub fn set_paused(&mut self, paused: bool) {
        self.get_mut().set_paused(paused)
    }

    /// Get if the media is paused or not.
    pub fn paused(&self) -> bool {
        self.read().paused()
    }

    /// Jumps to a specific position in the media.
    /// Passing `true` to the `accurate` parameter will result in more accurate seeking,
    /// however, it is also slower. For most seeks (e.g., scrubbing) this is not needed.
    pub fn seek(&mut self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        self.get_mut().seek(position, accurate)
    }

    /// Set the playback speed of the media.
    /// The default speed is `1.0`.
    pub fn set_speed(&mut self, speed: f64) -> Result<(), Error> {
        self.get_mut().set_speed(speed)
    }

    /// Get the current playback speed.
    pub fn speed(&self) -> f64 {
        self.read().speed
    }

    /// Get the current playback position in time.
    pub fn position(&self) -> Duration {
        Duration::from_nanos(
            self.read()
                .source
                .query_position::<gst::ClockTime>()
                .map_or(0, |pos| pos.nseconds()),
        )
    }

    /// Get the media duration.
    pub fn duration(&self) -> Duration {
        self.read().duration
    }

    /// Restarts a stream; seeks to the first frame and unpauses, sets the `eos` flag to false.
    pub fn restart_stream(&mut self) -> Result<(), Error> {
        self.get_mut().restart_stream()
    }

    /// Shows/Hides the subtitles on the media.
    pub fn toggle_subtitle(&mut self) {
        self.get_mut().toggle_subtitles()
    }

    /// Returns whether the subtitles is being shown or not.
    pub fn show_subtitles(&self) -> bool {
        self.read().show_subtitles
    }

    /// Returns the [`SubtitleFontDescription`] of the media.
    pub fn subtitle_description(&self) -> SubtitleFontDescription {
        self.read().subtitle_description
    }

    /// Sets the [`SubtitleFontDescription`] of the media.
    pub fn set_subtitle_description(&mut self, description: SubtitleFontDescription) {
        self.get_mut().set_subtitle_description(description)
    }

    /// Returns a list of available subtitles for the media.
    pub fn available_subtitles(&self) -> Vec<TextTag> {
        let pipeline = &self.read().source;
        let n = pipeline.property::<i32>("n-text");

        (0..n)
            .filter_map(|id| {
                let tags =
                    pipeline.emit_by_name::<Option<gst::TagList>>("get-text-tags", &[&id])?;
                let codec = tags.get::<gst::tags::LanguageCode>()?;

                Some(TextTag {
                    id,
                    language_code: codec.get().to_owned(),
                })
            })
            .collect()
    }

    /// Sets the subtitle to be shown for the media.
    pub fn set_text(&mut self, text: TextTag) {
        self.get_mut().set_text(text)
    }

    /// Gets the current subtitle of the media, if any.
    pub fn get_text(&self) -> Option<TextTag> {
        let pipeline = &self.read().source;

        let id = pipeline.property::<i32>("current-text");

        let tags = pipeline.emit_by_name::<Option<gst::TagList>>("get-text-tags", &[&id])?;
        let codec = tags.get::<gst::tags::LanguageCode>()?;

        Some(TextTag {
            id,
            language_code: codec.get().to_owned(),
        })
    }

    /// Set the subtitle URL to display.
    pub fn set_subtitle_url(&mut self, url: &url::Url) -> Result<(), Error> {
        let paused = self.paused();
        let mut inner = self.get_mut();
        inner.source.set_state(gst::State::Ready)?;
        inner.source.set_property("suburi", url.as_str());
        inner.set_paused(paused);
        Ok(())
    }

    /// Get the current subtitle URL.
    pub fn subtitle_url(&self) -> Option<url::Url> {
        url::Url::parse(&self.read().source.property::<String>("suburi")).ok()
    }

    /// Get the underlying GStreamer pipeline.
    pub fn pipeline(&self) -> gst::Pipeline {
        self.read().source.clone()
    }

    /// Generates a list of thumbnails based on a set of positions in the media, downscaled by a given factor.
    ///
    /// Slow; only needs to be called once for each instance.
    /// It's best to call this at the very start of playback, otherwise the position may shift.
    pub fn thumbnails<I>(
        &mut self,
        positions: I,
        downscale: NonZeroU8,
    ) -> Result<Vec<img::Handle>, Error>
    where
        I: IntoIterator<Item = Position>,
    {
        let downscale = u8::from(downscale) as u32;

        let paused = self.paused();
        let muted = self.muted();
        let pos = self.position();

        self.set_paused(false);
        self.set_muted(true);

        let out = {
            let inner = self.read();
            let width = inner.width;
            let height = inner.height;
            positions
                .into_iter()
                .map(|pos| {
                    inner.seek(pos, true)?;
                    inner.upload_frame.store(false, Ordering::SeqCst);
                    while !inner.upload_frame.load(Ordering::SeqCst) {
                        std::hint::spin_loop();
                    }
                    let frame_guard = inner.frame.lock().map_err(|_| Error::Lock)?;
                    let frame = frame_guard.readable().ok_or(Error::Lock)?;

                    Ok(img::Handle::from_rgba(
                        inner.width as u32 / downscale,
                        inner.height as u32 / downscale,
                        yuv_to_rgba(frame.as_slice(), width as _, height as _, downscale),
                    ))
                })
                .collect()
        };

        self.set_paused(paused);
        self.set_muted(muted);
        self.seek(pos, true)?;

        out
    }
}

fn yuv_to_rgba(yuv: &[u8], width: u32, height: u32, downscale: u32) -> Vec<u8> {
    let uv_start = width * height;
    let mut rgba = vec![];

    for y in 0..height / downscale {
        for x in 0..width / downscale {
            let x_src = x * downscale;
            let y_src = y * downscale;

            let uv_i = uv_start + width * (y_src / 2) + x_src / 2 * 2;

            let y = yuv[(y_src * width + x_src) as usize] as f32;
            let u = yuv[uv_i as usize] as f32;
            let v = yuv[(uv_i + 1) as usize] as f32;

            let r = 1.164 * (y - 16.0) + 1.596 * (v - 128.0);
            let g = 1.164 * (y - 16.0) - 0.813 * (v - 128.0) - 0.391 * (u - 128.0);
            let b = 1.164 * (y - 16.0) + 2.018 * (u - 128.0);

            rgba.push(r as u8);
            rgba.push(g as u8);
            rgba.push(b as u8);
            rgba.push(0xFF);
        }
    }

    rgba
}

#[derive(Debug, Clone)]
/// Subtitle meta data.
pub struct TextTag {
    id: i32,
    /// The language of the subtitle.
    pub language_code: String,
}

pub mod subtitles {
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    /// A font family.
    pub enum Family {
        Normal,
        #[default]
        Sans,
        Serif,
        Monospace,
    }

    impl Family {
        /// Returns a str representation of the [`Family`].
        pub fn to_str<'a>(self) -> &'a str {
            match self {
                Self::Normal => "Normal",
                Self::Sans => "Sans",
                Self::Serif => "Serif",
                Self::Monospace => "Monospace",
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    /// The style of the subtitle text.
    pub enum Style {
        #[default]
        Normal,
        Oblique,
        Italic,
    }

    impl Style {
        /// Returns a str representation of the [`Style`].
        pub fn to_str<'a>(self) -> &'a str {
            match self {
                Self::Normal => "Normal",
                Self::Italic => "Italic",
                Self::Oblique => "Oblique",
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    /// The weight of the subtitle text.
    pub enum Weight {
        Thin,
        Light,
        Regular,
        #[default]
        Medium,
        SemiBold,
        Bold,
        Black,
        Heavy,
    }

    impl Weight {
        /// Returns a str representation of the [`Weight`].
        pub fn to_str<'a>(self) -> &'a str {
            match self {
                Self::Thin => "Thin",
                Self::Light => "Light",
                Self::Regular => "Regular",
                Self::Medium => "Medium",
                Self::SemiBold => "SemiBold",
                Self::Bold => "Bold",
                Self::Black => "Black",
                Self::Heavy => "Heavy",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    /// Font rendering options for subtitles.
    pub struct SubtitleFontDescription {
        pub family: Family,
        pub style: Style,
        pub weight: Weight,
        pub size: u8,
    }

    impl Default for SubtitleFontDescription {
        fn default() -> Self {
            Self {
                size: 12,
                family: Family::default(),
                style: Style::default(),
                weight: Weight::default(),
            }
        }
    }

    impl std::fmt::Display for SubtitleFontDescription {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{} {} {} {}",
                self.family.to_str(),
                self.style.to_str(),
                self.weight.to_str(),
                self.size
            )
        }
    }
}
