{...}: {
  perSystem = {
    pkgs,
    config,
    lib,
    ...
  }: let
    gstreamer-plugins = with pkgs.gst_all_1; [gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly];
  in {
    # declare projects
    nci.projects."iced_video_player" = {
      path = ./.;

      runtimeLibs = with pkgs;
        [
          vulkan-loader
          wayland
          wayland-protocols
          libxkbcommon
          libx11
          libxrandr
          libxi
        ]
        ++ gstreamer-plugins;

      drvConfig.mkDerivation = {
        buildInputs = with pkgs; [pkg-config glib libxkbcommon] ++ gstreamer-plugins;
      };

      drvConfig.env = {
        "GST_PLUGIN_PATH" = lib.makeLibraryPath gstreamer-plugins;
      };
    };
    # configure crates
    nci.crates."iced_video_player" = {};
  };
}
