use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct StatusService {
    pub client: NightscoutClient,
}

impl StatusService {
    pub async fn get(&self) -> Result<Status, NightscoutError> {
        let url = self.client.base_url.join(Endpoint::Status.as_path())?;

        let mut request = self.client.http.get(url);
        request = self.client.auth(request);

        let response = self.client.send_checked(request).await?;

        Ok(response.json::<Status>().await?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct Status {
    pub status: String,
    #[allow(dead_code)]
    pub name: String,
    pub version: String,

    #[napi(js_name = "serverTime")]
    #[serde(rename = "serverTime")]
    pub server_time: String,

    #[napi(js_name = "serverTimeEpoch")]
    #[serde(rename = "serverTimeEpoch")]
    pub server_time_epoch: i64,

    #[napi(js_name = "apiEnabled")]
    #[serde(rename = "apiEnabled")]
    pub api_enabled: bool,

    #[napi(js_name = "carePortalEnabled")]
    #[serde(rename = "careportalEnabled")]
    pub care_portal_enabled: bool,

    #[napi(js_name = "bolusCalcEnabled")]
    #[serde(rename = "boluscalcEnabled")]
    pub bolus_calc_enabled: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<StatusSettings>,

    #[napi(js_name = "extendedSettings")]
    #[serde(
        default,
        rename = "extendedSettings",
        skip_serializing_if = "Option::is_none"
    )]
    pub extended_settings: Option<ExtendedSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorized: Option<bool>,

    #[napi(js_name = "runtimeState")]
    #[serde(
        default,
        rename = "runtimeState",
        skip_serializing_if = "Option::is_none"
    )]
    pub runtime_state: Option<String>,

    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct StatusSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    #[napi(js_name = "timeFormat")]
    #[serde(
        default,
        rename = "timeFormat",
        skip_serializing_if = "Option::is_none"
    )]
    pub time_format: Option<i64>,

    #[napi(js_name = "dayStart")]
    #[serde(default, rename = "dayStart", skip_serializing_if = "Option::is_none")]
    pub day_start: Option<i64>,

    #[napi(js_name = "dayEnd")]
    #[serde(default, rename = "dayEnd", skip_serializing_if = "Option::is_none")]
    pub day_end: Option<i64>,

    #[napi(js_name = "nightMode")]
    #[serde(default, rename = "nightMode", skip_serializing_if = "Option::is_none")]
    pub night_mode: Option<bool>,

    #[napi(js_name = "editMode")]
    #[serde(default, rename = "editMode", skip_serializing_if = "Option::is_none")]
    pub edit_mode: Option<bool>,

    #[napi(js_name = "showRawbg")]
    #[serde(default, rename = "showRawbg", skip_serializing_if = "Option::is_none")]
    pub show_rawbg: Option<String>,

    #[napi(js_name = "customTitle")]
    #[serde(
        default,
        rename = "customTitle",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_title: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,

    #[napi(js_name = "alarmUrgentHigh")]
    #[serde(
        default,
        rename = "alarmUrgentHigh",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_urgent_high: Option<bool>,

    #[napi(js_name = "alarmUrgentHighMins")]
    #[serde(
        default,
        rename = "alarmUrgentHighMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_urgent_high_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmHigh")]
    #[serde(default, rename = "alarmHigh", skip_serializing_if = "Option::is_none")]
    pub alarm_high: Option<bool>,

    #[napi(js_name = "alarmHighMins")]
    #[serde(
        default,
        rename = "alarmHighMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_high_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmLow")]
    #[serde(default, rename = "alarmLow", skip_serializing_if = "Option::is_none")]
    pub alarm_low: Option<bool>,

    #[napi(js_name = "alarmLowMins")]
    #[serde(
        default,
        rename = "alarmLowMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_low_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmUrgentLow")]
    #[serde(
        default,
        rename = "alarmUrgentLow",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_urgent_low: Option<bool>,

    #[napi(js_name = "alarmUrgentLowMins")]
    #[serde(
        default,
        rename = "alarmUrgentLowMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_urgent_low_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmUrgentMins")]
    #[serde(
        default,
        rename = "alarmUrgentMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_urgent_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmWarnMins")]
    #[serde(
        default,
        rename = "alarmWarnMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_warn_mins: Option<Vec<i64>>,

    #[napi(js_name = "alarmTimeagoWarn")]
    #[serde(
        default,
        rename = "alarmTimeagoWarn",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_timeago_warn: Option<bool>,

    #[napi(js_name = "alarmTimeagoWarnMins")]
    #[serde(
        default,
        rename = "alarmTimeagoWarnMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_timeago_warn_mins: Option<i64>,

    #[napi(js_name = "alarmTimeagoUrgent")]
    #[serde(
        default,
        rename = "alarmTimeagoUrgent",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_timeago_urgent: Option<bool>,

    #[napi(js_name = "alarmTimeagoUrgentMins")]
    #[serde(
        default,
        rename = "alarmTimeagoUrgentMins",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_timeago_urgent_mins: Option<i64>,

    #[napi(js_name = "alarmPumpBatteryLow")]
    #[serde(
        default,
        rename = "alarmPumpBatteryLow",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_pump_battery_low: Option<bool>,

    #[napi(js_name = "baseURL")]
    #[serde(default, rename = "baseURL", skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    #[napi(js_name = "authDefaultRoles")]
    #[serde(
        default,
        rename = "authDefaultRoles",
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_default_roles: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    #[napi(js_name = "scaleY")]
    #[serde(default, rename = "scaleY", skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<String>,

    #[napi(js_name = "showPlugins")]
    #[serde(
        default,
        rename = "showPlugins",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_plugins: Option<String>,

    #[napi(js_name = "showForecast")]
    #[serde(
        default,
        rename = "showForecast",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_forecast: Option<String>,

    #[napi(js_name = "focusHours")]
    #[serde(
        default,
        rename = "focusHours",
        skip_serializing_if = "Option::is_none"
    )]
    pub focus_hours: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<i64>,

    #[napi(js_name = "DEFAULT_FEATURES")]
    #[serde(
        default,
        rename = "DEFAULT_FEATURES",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_features: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thresholds: Option<StatusThresholds>,

    #[napi(js_name = "alarmTypes")]
    #[serde(
        default,
        rename = "alarmTypes",
        skip_serializing_if = "Option::is_none"
    )]
    pub alarm_types: Option<Vec<String>>,

    #[napi(js_name = "insecureUseHttp")]
    #[serde(
        default,
        rename = "insecureUseHttp",
        skip_serializing_if = "Option::is_none"
    )]
    pub insecure_use_http: Option<bool>,

    #[napi(js_name = "secureHstsHeader")]
    #[serde(
        default,
        rename = "secureHstsHeader",
        skip_serializing_if = "Option::is_none"
    )]
    pub secure_hsts_header: Option<bool>,

    #[napi(js_name = "secureHstsHeaderIncludeSubdomains")]
    #[serde(
        default,
        rename = "secureHstsHeaderIncludeSubdomains",
        skip_serializing_if = "Option::is_none"
    )]
    pub secure_hsts_header_include_subdomains: Option<bool>,

    #[napi(js_name = "secureHstsHeaderPreload")]
    #[serde(
        default,
        rename = "secureHstsHeaderPreload",
        skip_serializing_if = "Option::is_none"
    )]
    pub secure_hsts_header_preload: Option<bool>,

    #[napi(js_name = "secureCsp")]
    #[serde(default, rename = "secureCsp", skip_serializing_if = "Option::is_none")]
    pub secure_csp: Option<bool>,

    #[napi(js_name = "deNormalizeDates")]
    #[serde(
        default,
        rename = "deNormalizeDates",
        skip_serializing_if = "Option::is_none"
    )]
    pub de_normalize_dates: Option<bool>,

    #[napi(js_name = "showClockDelta")]
    #[serde(
        default,
        rename = "showClockDelta",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_clock_delta: Option<bool>,

    #[napi(js_name = "showClockLastTime")]
    #[serde(
        default,
        rename = "showClockLastTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_clock_last_time: Option<bool>,

    #[napi(js_name = "frameUrl1")]
    #[serde(default, rename = "frameUrl1", skip_serializing_if = "Option::is_none")]
    pub frame_url_1: Option<String>,

    #[napi(js_name = "frameUrl2")]
    #[serde(default, rename = "frameUrl2", skip_serializing_if = "Option::is_none")]
    pub frame_url_2: Option<String>,

    #[napi(js_name = "frameUrl3")]
    #[serde(default, rename = "frameUrl3", skip_serializing_if = "Option::is_none")]
    pub frame_url_3: Option<String>,

    #[napi(js_name = "frameUrl4")]
    #[serde(default, rename = "frameUrl4", skip_serializing_if = "Option::is_none")]
    pub frame_url_4: Option<String>,

    #[napi(js_name = "frameUrl5")]
    #[serde(default, rename = "frameUrl5", skip_serializing_if = "Option::is_none")]
    pub frame_url_5: Option<String>,

    #[napi(js_name = "frameUrl6")]
    #[serde(default, rename = "frameUrl6", skip_serializing_if = "Option::is_none")]
    pub frame_url_6: Option<String>,

    #[napi(js_name = "frameUrl7")]
    #[serde(default, rename = "frameUrl7", skip_serializing_if = "Option::is_none")]
    pub frame_url_7: Option<String>,

    #[napi(js_name = "frameUrl8")]
    #[serde(default, rename = "frameUrl8", skip_serializing_if = "Option::is_none")]
    pub frame_url_8: Option<String>,

    #[napi(js_name = "frameName1")]
    #[serde(
        default,
        rename = "frameName1",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_1: Option<String>,

    #[napi(js_name = "frameName2")]
    #[serde(
        default,
        rename = "frameName2",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_2: Option<String>,

    #[napi(js_name = "frameName3")]
    #[serde(
        default,
        rename = "frameName3",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_3: Option<String>,

    #[napi(js_name = "frameName4")]
    #[serde(
        default,
        rename = "frameName4",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_4: Option<String>,

    #[napi(js_name = "frameName5")]
    #[serde(
        default,
        rename = "frameName5",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_5: Option<String>,

    #[napi(js_name = "frameName6")]
    #[serde(
        default,
        rename = "frameName6",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_6: Option<String>,

    #[napi(js_name = "frameName7")]
    #[serde(
        default,
        rename = "frameName7",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_7: Option<String>,

    #[napi(js_name = "frameName8")]
    #[serde(
        default,
        rename = "frameName8",
        skip_serializing_if = "Option::is_none"
    )]
    pub frame_name_8: Option<String>,

    #[napi(js_name = "authFailDelay")]
    #[serde(
        default,
        rename = "authFailDelay",
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_fail_delay: Option<i64>,

    #[napi(js_name = "adminNotifiesEnabled")]
    #[serde(
        default,
        rename = "adminNotifiesEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    pub admin_notifies_enabled: Option<bool>,

    #[napi(js_name = "authenticationPromptOnLoad")]
    #[serde(
        default,
        rename = "authenticationPromptOnLoad",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_prompt_on_load: Option<bool>,

    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct StatusThresholds {
    #[napi(js_name = "bgHigh")]
    #[serde(default, rename = "bgHigh", skip_serializing_if = "Option::is_none")]
    pub bg_high: Option<i64>,

    #[napi(js_name = "bgTargetTop")]
    #[serde(
        default,
        rename = "bgTargetTop",
        skip_serializing_if = "Option::is_none"
    )]
    pub bg_target_top: Option<i64>,

    #[napi(js_name = "bgTargetBottom")]
    #[serde(
        default,
        rename = "bgTargetBottom",
        skip_serializing_if = "Option::is_none"
    )]
    pub bg_target_bottom: Option<i64>,

    #[napi(js_name = "bgLow")]
    #[serde(default, rename = "bgLow", skip_serializing_if = "Option::is_none")]
    pub bg_low: Option<i64>,

    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct ExtendedSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devicestatus: Option<ExtendedDeviceStatusSettings>,

    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct ExtendedDeviceStatusSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advanced: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
}
