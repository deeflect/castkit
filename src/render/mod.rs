pub mod screenstudio;
pub mod webstudio;

pub use screenstudio::{
    render_screenstudio, KeystrokeProfile, RenderArtifacts, RenderOptions, RenderOutputFormat,
    RenderSpeedPreset,
};
pub use webstudio::{build_web_manifest_preview, render_webstudio};
