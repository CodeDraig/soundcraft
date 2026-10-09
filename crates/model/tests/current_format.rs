//! The persistence contract is the complete current writer output, including nullable keys.
use serde::{Serialize, de::DeserializeOwned};
use soundcraft_model::*;

fn required_fields<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T, omitted: &[&str]) {
    let current = serde_json::to_value(&value).unwrap();
    assert_eq!(serde_json::from_value::<T>(current.clone()).unwrap(), value);
    for field in current.as_object().unwrap().keys().filter(|key| !omitted.contains(&key.as_str())) {
        let mut incomplete = current.clone();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<T>(incomplete).is_err(), "{} accepted missing {field}", std::any::type_name::<T>());
    }
}

#[test]
fn session_and_preset_objects_require_the_current_fields() {
    required_fields(Session::default(), &[]);
    required_fields(EditState::default(), &[]);
    required_fields(Track::new(TrackId(1), "Audio", TrackKind::Audio, ChannelFormat::Mono, [0, 0, 0]), &[]);
    required_fields(Clip::audio(ClipId(1), "clip", SourceId(1), 0, 0, 100), &[]);
    required_fields(Mixer::new(ChannelFormat::Stereo), &[]);
    required_fields(SendSlot::new(Route::Main), &[]);
    required_fields(
        Source {
            id: SourceId(1),
            name: "recording".into(),
            path: "recording.wav".into(),
            channels: 2,
            frames: 48_000,
            sample_rate: 48_000,
            format: soundcraft_audio_io::FileFormat::Wav,
            time_reference: 0,
            unsaved: false,
        },
        &[],
    );
    required_fields(
        VideoSource {
            id: SourceId(2),
            name: "picture".into(),
            path: "picture.mov".into(),
            width: 1920,
            height: 1080,
            frame_rate: 30.0,
            duration: 1.0,
            codec: "h264".into(),
        },
        &[],
    );
    required_fields(
        Group {
            id: GroupId(1),
            name: "Rhythm".into(),
            letter: 'a',
            members: vec![TrackId(1)],
            edit: true,
            mix: true,
            active: true,
            color: [1, 2, 3],
            attributes: vec!["volume".into()],
        },
        &[],
    );
    required_fields(SurroundPan::default(), &[]);
    required_fields(AutomationLane::new(AutoParam::Volume), &[]);
    required_fields(Insert::new("eq7"), &[]);
    let mut with_state = Insert::new("vst3:00");
    with_state.set_state_bytes(b"current state");
    required_fields(with_state, &["state"]);
    let mut session = Session::default();
    session.add_marker("Start", MarkerKind::Marker, 0, 0);
    required_fields(session.markers.remove(0), &[]);
}
