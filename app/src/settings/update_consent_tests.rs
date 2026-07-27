use settings_value::SettingsValue as _;

use super::UpdateConsent;

#[test]
fn the_default_is_unanswered() {
    // Critically NOT Enabled. The whole premise is that no network request happens until the
    // user has answered, so the default must be the state that makes no request AND shows the
    // prompt.
    assert_eq!(UpdateConsent::default(), UpdateConsent::Unanswered);
}

#[test]
fn only_an_explicit_yes_permits_a_network_request() {
    assert!(
        !UpdateConsent::Unanswered.should_check(),
        "not having asked is not consent"
    );
    assert!(
        !UpdateConsent::Disabled.should_check(),
        "declining must mean no request, ever"
    );
    assert!(UpdateConsent::Enabled.should_check());
}

#[test]
fn only_the_unanswered_state_prompts() {
    // Declining must not re-prompt on the next launch. That is the entire reason this is a
    // tri-state rather than a bool.
    assert!(UpdateConsent::Unanswered.needs_prompt());
    assert!(!UpdateConsent::Enabled.needs_prompt());
    assert!(!UpdateConsent::Disabled.needs_prompt());
}

#[test]
fn round_trips_through_the_settings_wire_format() {
    // The setting is persisted through `SettingsValue`, not plain serde, so the round trip
    // has to go through the same path the settings file uses. A format change that silently
    // reset this would re-prompt everyone and re-enable nobody.
    for value in [
        UpdateConsent::Unanswered,
        UpdateConsent::Enabled,
        UpdateConsent::Disabled,
    ] {
        let stored = value.to_file_value();
        let back = UpdateConsent::from_file_value(&stored)
            .unwrap_or_else(|| panic!("{value:?} must decode from its own encoding"));
        assert_eq!(value, back, "{value:?} must survive a round trip");
    }
}

#[test]
fn the_persisted_names_are_the_ones_we_expect() {
    // Pin the actual strings. `SettingsValue` derives its names from `#[serde(rename_all)]`,
    // and with none present it falls back to PascalCase -> snake_case -- so the file format
    // depends on an attribute that is easy to drop by accident. If someone removes the serde
    // attribute, this fails instead of silently orphaning every stored value.
    assert_eq!(
        UpdateConsent::Enabled.to_file_value(),
        serde_json::json!("enabled")
    );
    assert_eq!(
        UpdateConsent::Unanswered.to_file_value(),
        serde_json::json!("unanswered")
    );
    assert_eq!(
        UpdateConsent::Disabled.to_file_value(),
        serde_json::json!("disabled")
    );
}

#[test]
fn an_unknown_persisted_value_does_not_become_consent() {
    // A corrupt or future settings file must decode to `None` so the loader falls back to
    // the default. The direction matters far more than the mechanism: falling back to
    // `Unanswered` re-asks the user, whereas falling back to `Enabled` would manufacture
    // consent out of a malformed file.
    let decoded = UpdateConsent::from_file_value(&serde_json::json!("nonsense"));
    assert!(decoded.is_none(), "an unknown value must not decode");
    assert_eq!(
        decoded.unwrap_or_default(),
        UpdateConsent::Unanswered,
        "and the fallback must be the state that asks, not the state that permits"
    );
}
