use pianobar_core::{
    parse_playlist, parse_station_info, parse_stations, AudioFormat, AudioQuality, SongRating,
};

#[test]
fn fixture_station_list_sets_quickmix_membership() {
    let stations = parse_stations(include_str!("fixtures/stations_with_quickmix.json")).unwrap();

    assert_eq!(stations.len(), 3);
    assert_eq!(stations[0].name.as_deref(), Some("No Surprises Radio"));
    assert!(stations[0].is_creator);
    assert!(stations[0].use_quick_mix);
    assert!(stations[1].is_quick_mix);
    assert!(!stations[1].use_quick_mix);
    assert!(!stations[2].is_creator);
    assert!(stations[2].use_quick_mix);
}

#[test]
fn fixture_playlist_skips_ads_and_keeps_missing_audio_map_song() {
    let songs = parse_playlist(
        include_str!("fixtures/playlist_mixed_items.json"),
        AudioQuality::High,
    )
    .unwrap();

    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].title.as_deref(), Some("Keep on Loving You"));
    assert_eq!(songs[0].rating, SongRating::Love);
    assert_eq!(songs[0].audio_format, AudioFormat::AacPlus);
    assert_eq!(
        songs[0].audio_url.as_deref(),
        Some("https://audio.example/high.mp4")
    );
    assert_eq!(songs[1].title.as_deref(), Some("Halah"));
    assert_eq!(songs[1].audio_format, AudioFormat::Unknown);
    assert!(songs[1].audio_url.is_none());
}

#[test]
fn fixture_station_info_handles_sparse_seed_and_feedback_data() {
    let info = parse_station_info(include_str!("fixtures/station_info_sparse.json")).unwrap();

    assert_eq!(info.song_seeds.len(), 1);
    assert_eq!(info.artist_seeds.len(), 1);
    assert_eq!(info.feedback.len(), 2);
    assert_eq!(info.song_seeds[0].seed_id.as_deref(), Some("seed-song"));
    assert_eq!(info.artist_seeds[0].seed_id.as_deref(), Some("seed-artist"));
    assert_eq!(info.feedback[0].feedback_id.as_deref(), Some("feedback-up"));
    assert_eq!(info.feedback[0].rating, SongRating::Love);
    assert_eq!(info.feedback[0].length, 180);
    assert_eq!(
        info.feedback[1].feedback_id.as_deref(),
        Some("feedback-down")
    );
    assert_eq!(info.feedback[1].rating, SongRating::Ban);
    assert_eq!(info.feedback[1].length, 0);
}
