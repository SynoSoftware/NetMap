use crate::geography::Point;

mod dialog;
pub(super) mod storage;

pub use dialog::{REFRESH, show};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Countries,
    Cities,
}

pub use crate::map::SpeedCorner;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    Automatic,
    Manual(Point),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaverAction {
    Preview,
    Install,
    WindowsSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub detail: Detail,
    pub speeds: SpeedCorner,
    pub bright_seconds: u64,
    pub fade_seconds: u64,
    pub origin: Origin,
    pub desktop: bool,
    // Empty follows the current primary monitor; a name selects that display.
    pub monitor: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            detail: Detail::Countries,
            // TinyTorrent2 InspectorPane.MapView, revision 95be5335.
            speeds: SpeedCorner::BottomRight,
            bright_seconds: 10,
            fade_seconds: 20,
            origin: Origin::Automatic,
            desktop: false,
            monitor: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Map,
    Connections,
    Location,
    Desktop,
    ScreenSaver,
}

impl Page {
    pub const ALL: [Self; 5] = [
        Self::Map,
        Self::Connections,
        Self::Location,
        Self::Desktop,
        Self::ScreenSaver,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Map => "Map",
            Self::Connections => "Connections",
            Self::Location => "Location",
            Self::Desktop => "Desktop",
            Self::ScreenSaver => "Screen saver",
        }
    }

    pub fn captions(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Map => &[
                ("display", "Display"),
                ("detail", "&Detail:"),
                ("countries", "Countries"),
                ("cities", "Cities"),
                ("speeds", "&Speeds:"),
                ("top_left", "Top-left"),
                ("bottom_left", "Bottom-left"),
                ("bottom_right", "Bottom-right"),
                (
                    "appearance",
                    "Windows appearance and motion settings apply.",
                ),
                (
                    "cities_unavailable",
                    "Cities requires usable city data. Open Location to download it.",
                ),
                ("location", "&Location…"),
            ],
            Self::Connections => &[
                ("new_connections", "New connections"),
                ("bright", "&Bright for (seconds):"),
                ("fade", "&Fade out (seconds):"),
                ("traffic", "Traffic"),
                (
                    "activity",
                    "Measured send moves toward the destination; receive moves toward the origin. Traffic does not restart establishment brightness.",
                ),
            ],
            Self::Location => &[
                ("map_origin", "Map origin"),
                ("automatic", "&Automatic"),
                ("manual", "&Manual"),
                ("latitude", "&Latitude (north +):"),
                ("longitude", "L&ongitude (east +):"),
                (
                    "disclosure",
                    "Automatic requests the public egress address from api64.ipify.org and locates it locally. One overview origin may differ from individual VPN/proxy paths.",
                ),
                ("location_data", "Location data"),
                ("get_city", "&Get city data"),
                ("update_city", "&Update city data"),
                ("cancel_city", "Cancel &download"),
                (
                    "immediate",
                    "Data actions take effect immediately and continue if Settings closes.",
                ),
            ],
            Self::Desktop => &[
                ("background", "Desktop background"),
                ("enable", "&Enable desktop background"),
                ("monitor", "&Monitor:"),
                ("primary", "Primary monitor (automatic)"),
                (
                    "behavior",
                    "Closing the map keeps NetMap behind desktop icons while this option is enabled. Use the notification-area icon to reopen the map or Exit to stop NetMap.",
                ),
            ],
            Self::ScreenSaver => &[
                ("installation", "Installation and preview"),
                ("install", "&Install or update…"),
                ("windows", "Open &Windows screen saver settings…"),
                ("preview", "&Preview"),
                (
                    "immediate",
                    "These actions take effect immediately. Cancel does not undo installation or Windows settings. Installation may ask for administrator permission.",
                ),
            ],
        }
    }

    pub fn caption(self, key: &str) -> &'static str {
        self.captions().iter().find(|(id, _)| *id == key).unwrap().1
    }
}

// Word-prefix matching follows Goz's Settings catalogue, without its
// localization or unrelated application settings infrastructure.
fn words(text: &str) -> Vec<String> {
    text.replace('&', "")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub fn caption_matches(caption: &str, search: &str) -> bool {
    let caption = words(caption);
    words(search)
        .iter()
        .any(|term| caption.iter().any(|word| word.starts_with(term)))
}

pub fn matching_pages(search: &str) -> Vec<Page> {
    let terms = words(search);
    Page::ALL
        .into_iter()
        .filter(|page| {
            let synonyms = match page {
                Page::Map => "",
                Page::Connections => "dim brightness",
                Page::Location => "country dataset city dataset",
                Page::Desktop => "wallpaper background tray display",
                Page::ScreenSaver => "screensaver installation preview",
            };
            let vocabulary: Vec<_> = [page.title(), synonyms]
                .into_iter()
                .chain(page.captions().iter().map(|(_, caption)| *caption))
                .flat_map(words)
                .collect();
            terms
                .iter()
                .all(|term| vocabulary.iter().any(|word| word.starts_with(term)))
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Detail,
    Bright,
    Fade,
    Latitude,
    Longitude,
    Monitor,
}

impl Field {
    pub fn page(self) -> Page {
        match self {
            Self::Detail => Page::Map,
            Self::Bright | Self::Fade => Page::Connections,
            Self::Latitude | Self::Longitude => Page::Location,
            Self::Monitor => Page::Desktop,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Invalid {
    pub field: Field,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub detail: Detail,
    pub speeds: SpeedCorner,
    pub bright: String,
    pub fade: String,
    pub manual: bool,
    pub latitude: String,
    pub longitude: String,
    pub desktop: bool,
    pub monitor: String,
}

impl From<&Preferences> for Draft {
    fn from(value: &Preferences) -> Self {
        let point = match value.origin {
            Origin::Automatic => Point {
                latitude: 0.0,
                longitude: 0.0,
            },
            Origin::Manual(point) => point,
        };
        Self {
            detail: value.detail,
            speeds: value.speeds,
            bright: value.bright_seconds.to_string(),
            fade: value.fade_seconds.to_string(),
            manual: matches!(value.origin, Origin::Manual(_)),
            latitude: point.latitude.to_string(),
            longitude: point.longitude.to_string(),
            desktop: value.desktop,
            monitor: value.monitor.clone(),
        }
    }
}

impl Draft {
    pub fn reset(&mut self, page: Page) {
        let defaults = Self::from(&Preferences::default());
        match page {
            Page::Map => {
                self.detail = defaults.detail;
                self.speeds = defaults.speeds;
            }
            Page::Connections => {
                self.bright = defaults.bright;
                self.fade = defaults.fade;
            }
            Page::Location => {
                self.manual = defaults.manual;
                self.latitude = defaults.latitude;
                self.longitude = defaults.longitude;
            }
            Page::Desktop => {
                self.desktop = defaults.desktop;
                self.monitor = defaults.monitor;
            }
            Page::ScreenSaver => {}
        }
    }

    pub fn differs_from_defaults(&self, page: Page) -> bool {
        let mut reset = self.clone();
        reset.reset(page);
        *self != reset
    }

    pub fn validate(
        &self,
        city_available: bool,
        committed: &Preferences,
    ) -> Result<Preferences, Invalid> {
        if self.detail == Detail::Cities && committed.detail != Detail::Cities && !city_available {
            return Err(Invalid {
                field: Field::Detail,
                reason: "Get city data on the Location page before choosing Cities.",
            });
        }
        let seconds = |text: &str, field, positive: bool| {
            let text = text.trim();
            text.parse::<u64>()
                .ok()
                .filter(|value| !positive || *value != 0)
                .filter(|_| !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit()))
                .ok_or(Invalid {
                    field,
                    reason: if positive {
                        "Enter a positive whole number of seconds."
                    } else {
                        "Enter a whole number of seconds, or zero for immediate removal."
                    },
                })
        };
        let bright_seconds = seconds(&self.bright, Field::Bright, true)?;
        let fade_seconds = seconds(&self.fade, Field::Fade, false)?;
        // The shared frame clock represents elapsed time in u64 milliseconds.
        if bright_seconds
            .checked_add(fade_seconds)
            .and_then(|s| s.checked_mul(1000))
            .is_none()
        {
            return Err(Invalid {
                field: Field::Fade,
                reason: "The combined visibility duration is too large.",
            });
        }
        let coordinate = |text: &str, field, limit: f64| {
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && (-limit..=limit).contains(value))
                .ok_or(Invalid {
                    field,
                    reason: if field == Field::Latitude {
                        "Enter a finite latitude from -90 through 90."
                    } else {
                        "Enter a finite longitude from -180 through 180."
                    },
                })
        };
        let origin = if self.manual {
            Origin::Manual(Point {
                latitude: coordinate(&self.latitude, Field::Latitude, 90.0)?,
                longitude: coordinate(&self.longitude, Field::Longitude, 180.0)?,
            })
        } else {
            Origin::Automatic
        };
        if self.monitor.len() > 256 || self.monitor.chars().any(char::is_control) {
            return Err(Invalid {
                field: Field::Monitor,
                reason: "Choose a monitor from the available displays.",
            });
        }
        Ok(Preferences {
            detail: self.detail,
            speeds: self.speeds,
            bright_seconds,
            fade_seconds,
            origin,
            desktop: self.desktop,
            monitor: self.monitor.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub enum Phase {
    Loading,
    Ready,
    Saving { id: u64, preferences: Preferences },
}

#[derive(Clone)]
pub struct Settings {
    pub committed: Preferences,
    pub phase: Phase,
    pub error: Option<String>,
    generation: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            committed: Preferences::default(),
            phase: Phase::Loading,
            error: None,
            generation: 0,
        }
    }
}

impl Settings {
    pub fn last_save(&self) -> Option<(u64, bool)> {
        (matches!(self.phase, Phase::Ready) && self.generation != 0)
            .then_some((self.generation, self.error.is_none()))
    }

    pub fn loaded(&mut self, result: std::io::Result<Preferences>) {
        if !matches!(self.phase, Phase::Loading) {
            return;
        }
        match result {
            Ok(preferences) => self.committed = preferences,
            Err(error) => self.error = Some(format!("Could not read preferences: {error}")),
        }
        self.phase = Phase::Ready;
    }

    pub fn begin_save(&mut self, preferences: Preferences) -> Option<u64> {
        if !matches!(self.phase, Phase::Ready) {
            return None;
        }
        self.generation += 1;
        let id = self.generation;
        self.phase = Phase::Saving { id, preferences };
        self.error = None;
        Some(id)
    }

    pub fn saved(&mut self, id: u64, result: std::io::Result<()>) -> bool {
        if !matches!(self.phase, Phase::Saving { id: current, .. } if current == id) {
            return false;
        }
        let Phase::Saving { preferences, .. } = std::mem::replace(&mut self.phase, Phase::Ready)
        else {
            unreachable!()
        };
        match result {
            Ok(()) => {
                self.committed = preferences;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Could not save preferences: {error}")),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_requires_every_word_at_a_word_start() {
        assert_eq!(matching_pages("FADE bri"), [Page::Connections]);
        assert!(matching_pages("ade").is_empty());
        assert_eq!(matching_pages("  "), Page::ALL);
        assert!(caption_matches("L&ongitude (east +)", "long"));
    }

    #[test]
    fn reset_is_page_scoped_and_inactive_manual_text_survives() {
        let preferences = Preferences::default();
        let mut draft = Draft::from(&preferences);
        draft.latitude = "unfinished".into();
        draft.desktop = true;
        draft.monitor = r"\\.\DISPLAY8".into();
        draft.bright = "".into();
        draft.reset(Page::Connections);
        assert_eq!(draft.latitude, "unfinished");
        assert!(draft.desktop);
        assert_eq!(draft.monitor, r"\\.\DISPLAY8");
        draft.reset(Page::ScreenSaver);
        assert!(draft.desktop);
        draft.reset(Page::Desktop);
        assert_eq!(draft.monitor, "");
        assert_eq!(draft.validate(false, &preferences).unwrap(), preferences);
        draft.manual = true;
        assert_eq!(
            draft.validate(false, &preferences).unwrap_err().field,
            Field::Latitude
        );
        draft.reset(Page::Location);
        assert_eq!(draft, Draft::from(&preferences));
    }

    #[test]
    fn invalid_fields_are_ordered_and_duration_overflow_is_rejected() {
        let preferences = Preferences::default();
        let mut draft = Draft::from(&preferences);
        draft.bright = "0".into();
        draft.fade = "NaN".into();
        assert_eq!(
            draft.validate(false, &preferences).unwrap_err().field,
            Field::Bright
        );
        draft.bright = "1".into();
        draft.fade = u64::MAX.to_string();
        assert_eq!(
            draft.validate(false, &preferences).unwrap_err().field,
            Field::Fade
        );
        draft.fade = "0".into();
        draft.manual = true;
        draft.latitude = "NaN".into();
        assert_eq!(
            draft.validate(false, &preferences).unwrap_err().field,
            Field::Latitude
        );
    }

    #[test]
    fn failed_or_stale_save_cannot_publish_a_snapshot() {
        let mut settings = Settings::default();
        settings.loaded(Ok(Preferences::default()));
        let desired = Preferences {
            bright_seconds: 30,
            ..Preferences::default()
        };
        let id = settings.begin_save(desired.clone()).unwrap();
        assert!(settings.begin_save(desired.clone()).is_none());
        assert!(!settings.saved(id + 1, Ok(())));
        settings.saved(id, Err(std::io::Error::other("write failed")));
        assert_eq!(settings.committed, Preferences::default());
        let id = settings.begin_save(desired.clone()).unwrap();
        settings.saved(id, Ok(()));
        assert_eq!(settings.committed, desired);
    }
}
