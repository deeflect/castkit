use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct BrandingConfig {
    pub title: Option<String>,
    pub bg_primary: Option<String>,
    pub bg_secondary: Option<String>,
    pub text_primary: Option<String>,
    pub text_muted: Option<String>,
    pub command_text: Option<String>,
    pub accent: Option<String>,
    pub watermark_text: Option<String>,
    pub avatar_x: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_label: Option<String>,
}

impl BrandingConfig {
    pub fn overlay(self, top: BrandingConfig) -> BrandingConfig {
        BrandingConfig {
            title: top.title.or(self.title),
            bg_primary: top.bg_primary.or(self.bg_primary),
            bg_secondary: top.bg_secondary.or(self.bg_secondary),
            text_primary: top.text_primary.or(self.text_primary),
            text_muted: top.text_muted.or(self.text_muted),
            command_text: top.command_text.or(self.command_text),
            accent: top.accent.or(self.accent),
            watermark_text: top.watermark_text.or(self.watermark_text),
            avatar_x: top.avatar_x.or(self.avatar_x),
            avatar_url: top.avatar_url.or(self.avatar_url),
            avatar_label: top.avatar_label.or(self.avatar_label),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.bg_primary.is_none()
            && self.bg_secondary.is_none()
            && self.text_primary.is_none()
            && self.text_muted.is_none()
            && self.command_text.is_none()
            && self.accent.is_none()
            && self.watermark_text.is_none()
            && self.avatar_x.is_none()
            && self.avatar_url.is_none()
            && self.avatar_label.is_none()
    }
}
