//! The `treatments` collection: boluses, carbs, temp basals, targets, site changes, notes…

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{CollectionName, Extra, Meta, Timestamp, de, impl_document, time};

/// A treatment `eventType`.
///
/// Covers Nightscout's careportal, the OpenAPS/AAPS and Loop plugins; any other value is
/// preserved in [`EventType::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EventType {
    /// `<none>`: the careportal's "no event type".
    Unspecified,
    /// `BG Check`.
    BgCheck,
    /// `Snack Bolus`.
    SnackBolus,
    /// `Meal Bolus`.
    MealBolus,
    /// `Correction Bolus` (also used for SMBs by AAPS, with `isSMB`).
    CorrectionBolus,
    /// `Carb Correction`.
    CarbCorrection,
    /// `Combo Bolus` (extended/dual-wave).
    ComboBolus,
    /// `Bolus Wizard`.
    BolusWizard,
    /// `Announcement`.
    Announcement,
    /// `Note`.
    Note,
    /// `Question`.
    Question,
    /// `Exercise`.
    Exercise,
    /// `Site Change` (feeds CAGE).
    SiteChange,
    /// `Sensor Start` (feeds SAGE).
    SensorStart,
    /// `Sensor Change` (feeds SAGE).
    SensorChange,
    /// `Sensor Stop`.
    SensorStop,
    /// `Pump Battery Change` (feeds BAGE).
    PumpBatteryChange,
    /// `Insulin Change` (feeds IAGE).
    InsulinChange,
    /// `Temp Basal`.
    TempBasal,
    /// `Temp Basal Start` (careportal; normalized to `Temp Basal` by Nightscout's UI).
    TempBasalStart,
    /// `Temp Basal End`.
    TempBasalEnd,
    /// `Profile Switch`.
    ProfileSwitch,
    /// `Effective Profile Switch` (AAPS).
    EffectiveProfileSwitch,
    /// `D.A.D. Alert` (diabetes alert dog).
    DadAlert,
    /// `Temporary Target`.
    TemporaryTarget,
    /// `Temporary Target Cancel`.
    TemporaryTargetCancel,
    /// `OpenAPS Offline`.
    OpenApsOffline,
    /// `Temporary Override` (Loop).
    TemporaryOverride,
    /// `Temporary Override Cancel` (Loop).
    TemporaryOverrideCancel,
    /// `Remote Carbs Entry` (Loop remote commands).
    RemoteCarbsEntry,
    /// `Remote Bolus Entry` (Loop remote commands).
    RemoteBolusEntry,
    /// `Suspend Pump` (Loop).
    SuspendPump,
    /// `Resume Pump` (Loop).
    ResumePump,
    /// Any other value, kept verbatim.
    Other(String),
}

const EVENT_TYPES: &[(EventType, &str)] = &[
    (EventType::Unspecified, "<none>"),
    (EventType::BgCheck, "BG Check"),
    (EventType::SnackBolus, "Snack Bolus"),
    (EventType::MealBolus, "Meal Bolus"),
    (EventType::CorrectionBolus, "Correction Bolus"),
    (EventType::CarbCorrection, "Carb Correction"),
    (EventType::ComboBolus, "Combo Bolus"),
    (EventType::BolusWizard, "Bolus Wizard"),
    (EventType::Announcement, "Announcement"),
    (EventType::Note, "Note"),
    (EventType::Question, "Question"),
    (EventType::Exercise, "Exercise"),
    (EventType::SiteChange, "Site Change"),
    (EventType::SensorStart, "Sensor Start"),
    (EventType::SensorChange, "Sensor Change"),
    (EventType::SensorStop, "Sensor Stop"),
    (EventType::PumpBatteryChange, "Pump Battery Change"),
    (EventType::InsulinChange, "Insulin Change"),
    (EventType::TempBasal, "Temp Basal"),
    (EventType::TempBasalStart, "Temp Basal Start"),
    (EventType::TempBasalEnd, "Temp Basal End"),
    (EventType::ProfileSwitch, "Profile Switch"),
    (
        EventType::EffectiveProfileSwitch,
        "Effective Profile Switch",
    ),
    (EventType::DadAlert, "D.A.D. Alert"),
    (EventType::TemporaryTarget, "Temporary Target"),
    (EventType::TemporaryTargetCancel, "Temporary Target Cancel"),
    (EventType::OpenApsOffline, "OpenAPS Offline"),
    (EventType::TemporaryOverride, "Temporary Override"),
    (
        EventType::TemporaryOverrideCancel,
        "Temporary Override Cancel",
    ),
    (EventType::RemoteCarbsEntry, "Remote Carbs Entry"),
    (EventType::RemoteBolusEntry, "Remote Bolus Entry"),
    (EventType::SuspendPump, "Suspend Pump"),
    (EventType::ResumePump, "Resume Pump"),
];

impl EventType {
    /// Parses an `eventType`; unknown values become [`EventType::Other`].
    #[must_use]
    pub fn parse(s: &str) -> Self {
        EVENT_TYPES
            .iter()
            .find(|(_, name)| *name == s)
            .map_or_else(|| Self::Other(s.to_owned()), |(ty, _)| ty.clone())
    }

    /// The exact string Nightscout stores.
    #[must_use]
    pub fn as_str(&self) -> &str {
        if let Self::Other(s) = self {
            return s;
        }
        EVENT_TYPES
            .iter()
            .find(|(ty, _)| ty == self)
            .map_or("", |(_, name)| name)
    }

    /// Whether the event records a bolus.
    #[must_use]
    pub const fn is_bolus(&self) -> bool {
        matches!(
            self,
            Self::SnackBolus
                | Self::MealBolus
                | Self::CorrectionBolus
                | Self::ComboBolus
                | Self::BolusWizard
        )
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for EventType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self::parse(&s))
    }
}

/// A care event.
///
/// Fields are optional because each [`EventType`] uses a different subset; see
/// [`Treatment::kind`] for a typed view. Construct with the helpers
/// ([`Treatment::carbs`], [`Treatment::bolus`], [`Treatment::temp_basal_absolute`], …).
///
/// ```
/// use cinnamon::model::{Treatment, EventType};
///
/// let snack = Treatment::carbs(15.0).with_notes("apple");
/// assert_eq!(snack.event_type, Some(EventType::CarbCorrection));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Treatment {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// What happened.
    #[serde(
        rename = "eventType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub event_type: Option<EventType>,
    /// When it happened (ISO string on the wire).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// When it happened, as epoch millis (set by API v3 and AAPS).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub date: Option<Timestamp>,
    /// The uploading device.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// Who entered it (person or app).
    #[serde(
        rename = "enteredBy",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub entered_by: Option<String>,
    /// Free-text notes.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub notes: Option<String>,
    /// Reason (temporary targets, overrides).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub reason: Option<String>,
    /// Blood glucose recorded with the event (in `units`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub glucose: Option<f64>,
    /// `Finger` or `Sensor`.
    #[serde(
        rename = "glucoseType",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub glucose_type: Option<String>,
    /// Units of `glucose` and targets (`mg/dl` or `mmol`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub units: Option<String>,
    /// Carbohydrates in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub carbs: Option<f64>,
    /// Protein in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub protein: Option<f64>,
    /// Fat in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub fat: Option<f64>,
    /// Expected carb absorption time in minutes (Loop).
    #[serde(
        rename = "absorptionTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub absorption_time: Option<f64>,
    /// Insulin in units.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub insulin: Option<f64>,
    /// Whether the bolus was a super micro bolus (AAPS/Trio).
    #[serde(
        rename = "isSMB",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::flag"
    )]
    pub is_smb: Option<bool>,
    /// Duration in minutes (temp basals, targets, exercise, profile switches, …).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub duration: Option<f64>,
    /// Duration in milliseconds (AAPS).
    #[serde(
        rename = "durationInMilliseconds",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::int"
    )]
    pub duration_in_milliseconds: Option<i64>,
    /// Absolute temp basal rate in U/h.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub absolute: Option<f64>,
    /// Temp basal rate in U/h as reported by some uploaders.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub rate: Option<f64>,
    /// Relative temp basal change in percent (`-20` = 80 % of the profile basal).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub percent: Option<f64>,
    /// Extended bolus rate in U/h (combo bolus).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub relative: Option<f64>,
    /// Upper bound of a temporary target.
    #[serde(
        rename = "targetTop",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub target_top: Option<f64>,
    /// Lower bound of a temporary target.
    #[serde(
        rename = "targetBottom",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub target_bottom: Option<f64>,
    /// Profile name (profile switch).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub profile: Option<String>,
    /// Profile percentage (profile switch).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub percentage: Option<f64>,
    /// Profile time shift in hours (profile switch).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub timeshift: Option<f64>,
    /// JSON-encoded profile carried by an AAPS profile switch.
    #[serde(
        rename = "profileJson",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub profile_json: Option<String>,
    /// Percent delivered immediately (combo bolus).
    #[serde(
        rename = "splitNow",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub split_now: Option<f64>,
    /// Percent delivered extended (combo bolus).
    #[serde(
        rename = "splitExt",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub split_ext: Option<f64>,
    /// Minutes between bolus and carbs.
    #[serde(
        rename = "preBolus",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub pre_bolus: Option<f64>,
    /// Set by Nightscout on announcements.
    #[serde(
        rename = "isAnnouncement",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::flag"
    )]
    pub is_announcement: Option<bool>,
    /// Sensor code (sensor start/change).
    #[serde(
        rename = "sensorCode",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub sensor_code: Option<String>,
    /// Transmitter id (sensor start/change).
    #[serde(
        rename = "transmitterId",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub transmitter_id: Option<String>,
    /// Pump record id (AAPS).
    #[serde(
        rename = "pumpId",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::int"
    )]
    pub pump_id: Option<i64>,
    /// Pump model (AAPS).
    #[serde(
        rename = "pumpType",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub pump_type: Option<String>,
    /// Pump serial (AAPS).
    #[serde(
        rename = "pumpSerial",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub pump_serial: Option<String>,
    /// Bolus wizard inputs, as stored by the careportal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boluscalc: Option<serde_json::Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Treatment {
    /// A treatment of `event_type`, dated now.
    #[must_use]
    pub fn new(event_type: EventType) -> Self {
        Self {
            meta: Meta::default(),
            event_type: Some(event_type),
            created_at: Some(Timestamp::now()),
            date: None,
            device: None,
            entered_by: None,
            notes: None,
            reason: None,
            glucose: None,
            glucose_type: None,
            units: None,
            carbs: None,
            protein: None,
            fat: None,
            absorption_time: None,
            insulin: None,
            is_smb: None,
            duration: None,
            duration_in_milliseconds: None,
            absolute: None,
            rate: None,
            percent: None,
            relative: None,
            target_top: None,
            target_bottom: None,
            profile: None,
            percentage: None,
            timeshift: None,
            profile_json: None,
            split_now: None,
            split_ext: None,
            pre_bolus: None,
            is_announcement: None,
            sensor_code: None,
            transmitter_id: None,
            pump_id: None,
            pump_type: None,
            pump_serial: None,
            boluscalc: None,
            extra: Extra::new(),
        }
    }

    /// A correction bolus of `units` U.
    #[must_use]
    pub fn bolus(units: f64) -> Self {
        Self {
            insulin: Some(units),
            ..Self::new(EventType::CorrectionBolus)
        }
    }

    /// A meal bolus: `units` U for `carbs` g.
    #[must_use]
    pub fn meal_bolus(units: f64, carbs: f64) -> Self {
        Self {
            insulin: Some(units),
            carbs: Some(carbs),
            ..Self::new(EventType::MealBolus)
        }
    }

    /// Carbs without insulin (`Carb Correction`).
    #[must_use]
    pub fn carbs(grams: f64) -> Self {
        Self {
            carbs: Some(grams),
            ..Self::new(EventType::CarbCorrection)
        }
    }

    /// An absolute temp basal of `rate` U/h for `minutes`.
    #[must_use]
    pub fn temp_basal_absolute(rate: f64, minutes: f64) -> Self {
        Self {
            absolute: Some(rate),
            duration: Some(minutes),
            ..Self::new(EventType::TempBasal)
        }
    }

    /// A relative temp basal (`percent` change from profile, e.g. `-20`) for `minutes`.
    #[must_use]
    pub fn temp_basal_percent(percent: f64, minutes: f64) -> Self {
        Self {
            percent: Some(percent),
            duration: Some(minutes),
            ..Self::new(EventType::TempBasal)
        }
    }

    /// A temporary target between `bottom` and `top` (mg/dL) for `minutes`.
    #[must_use]
    pub fn temp_target(bottom: f64, top: f64, minutes: f64) -> Self {
        Self {
            target_bottom: Some(bottom),
            target_top: Some(top),
            duration: Some(minutes),
            units: Some("mg/dl".into()),
            ..Self::new(EventType::TemporaryTarget)
        }
    }

    /// Switches to the profile named `profile` (permanently until the next switch).
    #[must_use]
    pub fn profile_switch(profile: impl Into<String>) -> Self {
        Self {
            profile: Some(profile.into()),
            duration: Some(0.0),
            ..Self::new(EventType::ProfileSwitch)
        }
    }

    /// A finger-stick (or sensor) blood glucose check.
    #[must_use]
    pub fn bg_check(glucose: f64, units: super::Units, finger: bool) -> Self {
        Self {
            glucose: Some(glucose),
            glucose_type: Some(if finger { "Finger" } else { "Sensor" }.into()),
            units: Some(units.as_str().into()),
            ..Self::new(EventType::BgCheck)
        }
    }

    /// A note.
    #[must_use]
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            notes: Some(text.into()),
            ..Self::new(EventType::Note)
        }
    }

    /// An announcement shown to everyone watching the site.
    #[must_use]
    pub fn announcement(text: impl Into<String>) -> Self {
        Self {
            notes: Some(text.into()),
            ..Self::new(EventType::Announcement)
        }
    }

    /// Exercise lasting `minutes`.
    #[must_use]
    pub fn exercise(minutes: f64) -> Self {
        Self {
            duration: Some(minutes),
            ..Self::new(EventType::Exercise)
        }
    }

    /// A pump site / cannula change.
    #[must_use]
    pub fn site_change() -> Self {
        Self::new(EventType::SiteChange)
    }

    /// A new CGM sensor was started.
    #[must_use]
    pub fn sensor_start() -> Self {
        Self::new(EventType::SensorStart)
    }

    /// A CGM sensor was replaced.
    #[must_use]
    pub fn sensor_change() -> Self {
        Self::new(EventType::SensorChange)
    }

    /// The insulin cartridge/reservoir was changed.
    #[must_use]
    pub fn insulin_change() -> Self {
        Self::new(EventType::InsulinChange)
    }

    /// The pump battery was changed.
    #[must_use]
    pub fn pump_battery_change() -> Self {
        Self::new(EventType::PumpBatteryChange)
    }

    /// Sets when the event happened.
    #[must_use]
    pub fn at(mut self, time: impl Into<Timestamp>) -> Self {
        self.created_at = Some(time.into());
        self.date = None;
        self
    }

    /// Sets the notes.
    #[must_use]
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Sets who entered the event.
    #[must_use]
    pub fn with_entered_by(mut self, who: impl Into<String>) -> Self {
        self.entered_by = Some(who.into());
        self
    }

    /// Sets the device.
    #[must_use]
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }

    /// Sets the duration in minutes.
    #[must_use]
    pub const fn with_duration(mut self, minutes: f64) -> Self {
        self.duration = Some(minutes);
        self
    }

    /// Sets the reason (temporary targets, overrides).
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// When the event happened: `created_at`, falling back to `date`.
    #[must_use]
    pub fn time(&self) -> Option<Timestamp> {
        self.created_at.or(self.date)
    }

    /// The event's duration in minutes, from `duration` or `durationInMilliseconds`.
    #[must_use]
    pub fn duration_minutes(&self) -> Option<f64> {
        self.duration
            .or_else(|| self.duration_in_milliseconds.map(|ms| ms as f64 / 60_000.0))
    }

    /// A typed view of the treatment, based on its event type and fields.
    #[must_use]
    pub fn kind(&self) -> TreatmentKind<'_> {
        let Some(event) = &self.event_type else {
            return self.untyped_kind();
        };
        match event {
            EventType::TempBasal | EventType::TempBasalStart => TreatmentKind::TempBasal {
                rate: self.absolute.or(self.rate),
                percent: self.percent,
                minutes: self.duration_minutes(),
            },
            EventType::TempBasalEnd => TreatmentKind::TempBasal {
                rate: None,
                percent: None,
                minutes: Some(0.0),
            },
            EventType::TemporaryTarget => TreatmentKind::TempTarget {
                bottom: self.target_bottom,
                top: self.target_top,
                minutes: self.duration_minutes(),
                reason: self.reason.as_deref(),
            },
            EventType::TemporaryTargetCancel => TreatmentKind::TempTarget {
                bottom: None,
                top: None,
                minutes: Some(0.0),
                reason: self.reason.as_deref(),
            },
            EventType::ProfileSwitch | EventType::EffectiveProfileSwitch => {
                TreatmentKind::ProfileSwitch {
                    profile: self.profile.as_deref(),
                    percentage: self.percentage,
                    timeshift: self.timeshift,
                    minutes: self.duration_minutes(),
                }
            }
            EventType::BgCheck => TreatmentKind::BgCheck {
                glucose: self.glucose,
                units: self.units.as_deref(),
            },
            EventType::SiteChange => TreatmentKind::SiteChange,
            EventType::SensorStart | EventType::SensorChange => TreatmentKind::SensorChange,
            EventType::InsulinChange => TreatmentKind::InsulinChange,
            EventType::PumpBatteryChange => TreatmentKind::PumpBatteryChange,
            EventType::Note | EventType::Announcement | EventType::Question => {
                TreatmentKind::Note {
                    text: self.notes.as_deref(),
                }
            }
            EventType::Exercise => TreatmentKind::Exercise {
                minutes: self.duration_minutes(),
            },
            _ => self.untyped_kind(),
        }
    }

    fn untyped_kind(&self) -> TreatmentKind<'_> {
        match (
            self.insulin.filter(|i| *i > 0.0),
            self.carbs.filter(|c| *c > 0.0),
        ) {
            (Some(insulin), carbs) => TreatmentKind::Bolus {
                insulin,
                carbs,
                smb: self.is_smb.unwrap_or(false),
            },
            (None, Some(grams)) => TreatmentKind::Carbs { grams },
            (None, None) => TreatmentKind::Other,
        }
    }

    fn fill_defaults(&mut self) {
        match (self.created_at, self.date) {
            (None, None) => {
                let now = Timestamp::now();
                self.created_at = Some(now);
                self.date = Some(now);
            }
            (Some(t), None) => self.date = Some(t),
            (None, Some(t)) => self.created_at = Some(t),
            (Some(_), Some(_)) => {}
        }
    }
}

impl_document!(
    Treatment,
    CollectionName::Treatments,
    timestamp = |s| s.time(),
    device = |s| s.device.as_deref(),
    event_type = |s| s.event_type.as_ref().map(EventType::as_str)
);

/// A typed view of a [`Treatment`] (see [`Treatment::kind`]).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TreatmentKind<'a> {
    /// Insulin delivered, optionally with carbs.
    Bolus {
        /// Units of insulin.
        insulin: f64,
        /// Grams of carbs recorded with the bolus.
        carbs: Option<f64>,
        /// Whether it was a super micro bolus.
        smb: bool,
    },
    /// Carbs without insulin.
    Carbs {
        /// Grams of carbs.
        grams: f64,
    },
    /// A temporary basal rate; `minutes == Some(0.0)` ends the running one.
    TempBasal {
        /// Absolute rate in U/h.
        rate: Option<f64>,
        /// Relative change in percent.
        percent: Option<f64>,
        /// Duration in minutes.
        minutes: Option<f64>,
    },
    /// A temporary glucose target; `minutes == Some(0.0)` cancels the running one.
    TempTarget {
        /// Lower bound.
        bottom: Option<f64>,
        /// Upper bound.
        top: Option<f64>,
        /// Duration in minutes.
        minutes: Option<f64>,
        /// Reason given.
        reason: Option<&'a str>,
    },
    /// A profile switch.
    ProfileSwitch {
        /// Profile name.
        profile: Option<&'a str>,
        /// Percentage applied to the profile.
        percentage: Option<f64>,
        /// Time shift in hours.
        timeshift: Option<f64>,
        /// Duration in minutes (0 = permanent).
        minutes: Option<f64>,
    },
    /// A glucose check.
    BgCheck {
        /// The measured value.
        glucose: Option<f64>,
        /// Its units.
        units: Option<&'a str>,
    },
    /// Pump site change.
    SiteChange,
    /// CGM sensor start or change.
    SensorChange,
    /// Insulin reservoir change.
    InsulinChange,
    /// Pump battery change.
    PumpBatteryChange,
    /// A note, announcement or question.
    Note {
        /// The text.
        text: Option<&'a str>,
    },
    /// Exercise.
    Exercise {
        /// Duration in minutes.
        minutes: Option<f64>,
    },
    /// Anything else.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types_round_trip() {
        for (ty, name) in EVENT_TYPES {
            assert_eq!(EventType::parse(name), *ty);
            assert_eq!(ty.as_str(), *name);
        }
        assert_eq!(EventType::parse("Custom Thing").as_str(), "Custom Thing");
    }

    #[test]
    fn aaps_smb_round_trips_losslessly() {
        let raw = serde_json::json!({
            "identifier": "4e1c9a3c-0b4b-4c1e-9d6a-3c3a1f2b9e11",
            "eventType": "Correction Bolus",
            "created_at": "2024-05-01T12:00:00.000Z",
            "date": 1714564800000_i64,
            "utcOffset": 120,
            "app": "AAPS",
            "insulin": 0.35,
            "type": "SMB",
            "isSMB": true,
            "pumpId": 4102,
            "pumpType": "OMNIPOD_DASH",
            "pumpSerial": "4241",
            "isValid": true,
            "srvModified": 1714564801234_i64,
            "srvCreated": 1714564801234_i64,
            "subject": "aaps"
        });
        let t: Treatment = serde_json::from_value(raw.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            t.kind(),
            TreatmentKind::Bolus {
                insulin: 0.35,
                carbs: None,
                smb: true
            }
        );
        assert_eq!(serde_json::to_value(&t).ok(), Some(raw));
    }

    #[test]
    fn careportal_nulls_do_not_break_decoding() {
        let t: Treatment = serde_json::from_value(serde_json::json!({
            "_id": "65f1c2a8e4b0a1b2c3d4e5f6",
            "eventType": "Temp Basal",
            "created_at": "2024-05-01T12:00:00.000Z",
            "absolute": "0.85",
            "duration": 30,
            "carbs": null,
            "insulin": null
        }))
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            t.kind(),
            TreatmentKind::TempBasal {
                rate: Some(0.85),
                percent: None,
                minutes: Some(30.0)
            }
        );
    }
}
