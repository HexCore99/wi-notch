use ::windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as WindowsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use ::windows::Storage::Streams::IRandomAccessStreamReference;
use chrono::Utc;

use super::streams::read_image_stream;

pub struct SessionManager(WindowsSessionManager);

impl SessionManager {
    pub async fn connect() -> Option<Self> {
        let manager = WindowsSessionManager::RequestAsync().ok()?.await.ok()?;
        Some(Self(manager))
    }
}

#[derive(Clone, Copy)]
pub enum MediaAction {
    Previous,
    Play,
    Pause,
    Next,
}

#[derive(Clone)]
pub struct NowPlaying {
    session: Session,
    source_app: String,
    album: String,
    track_number: i32,
    thumbnail: Option<IRandomAccessStreamReference>,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_previous: bool,
    pub can_next: bool,
    pub elapsed: u64,
    pub duration: u64,
}

impl NowPlaying {
    pub async fn read(manager: &SessionManager) -> Option<Self> {
        let session = manager.0.GetCurrentSession().ok()?;
        let media = session.TryGetMediaPropertiesAsync().ok()?.await.ok()?;
        let playback = session.GetPlaybackInfo().ok()?;
        let controls = playback.Controls().ok()?;
        let playing = playback.PlaybackStatus().ok()? == PlaybackStatus::Playing;
        let rate = playback
            .PlaybackRate()
            .and_then(|rate| rate.Value())
            .unwrap_or(1.0);
        let (elapsed, duration) = timeline(&session, playing, rate).unwrap_or_default();

        Some(Self {
            source_app: session
                .SourceAppUserModelId()
                .unwrap_or_default()
                .to_string(),
            session,
            album: media.AlbumTitle().unwrap_or_default().to_string(),
            track_number: media.TrackNumber().unwrap_or_default(),
            thumbnail: media.Thumbnail().ok(),
            title: media.Title().unwrap_or_default().to_string(),
            artist: media.Artist().unwrap_or_default().to_string(),
            playing,
            can_play: controls.IsPlayEnabled().unwrap_or(false),
            can_pause: controls.IsPauseEnabled().unwrap_or(false),
            can_previous: controls.IsPreviousEnabled().unwrap_or(false),
            can_next: controls.IsNextEnabled().unwrap_or(false),
            elapsed,
            duration,
        })
    }

    pub fn has_same_artwork(&self, other: &Self) -> bool {
        self.source_app == other.source_app
            && self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.track_number == other.track_number
            && self.thumbnail.is_some() == other.thumbnail.is_some()
    }

    pub async fn read_artwork(&self) -> Option<(String, Vec<u8>)> {
        let stream = self.thumbnail.as_ref()?.OpenReadAsync().ok()?.await.ok()?;
        read_image_stream(stream, 8 * 1024 * 1024).await
    }

    pub async fn send_action(&self, action: MediaAction) -> ::windows::core::Result<bool> {
        match action {
            MediaAction::Previous => self.session.TrySkipPreviousAsync(),
            MediaAction::Play => self.session.TryPlayAsync(),
            MediaAction::Pause => self.session.TryPauseAsync(),
            MediaAction::Next => self.session.TrySkipNextAsync(),
        }?
        .await
    }
}

fn timeline(session: &Session, playing: bool, rate: f64) -> ::windows::core::Result<(u64, u64)> {
    const TICKS_PER_SECOND: i64 = 10_000_000; //1 epoch  =100 nanosecond

    // windows time start at 1601, chrono start at 1970. This line fixed that gap
    const WINDOWS_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

    let timeline = session.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let duration = (timeline.EndTime()?.Duration - start).max(0);
    let mut position = timeline.Position()?.Duration - start;

    // Windows reports the position at the last update, which may be seconds old.
    let last_update = timeline.LastUpdatedTime()?.UniversalTime;
    if playing && last_update > 0 {
        let now = (Utc::now().timestamp_millis() + WINDOWS_EPOCH_OFFSET_MS) * 10_000;
        let ticks_since_update = (now - last_update).max(0);
        position += (ticks_since_update as f64 * rate) as i64;
    }

    Ok((
        (position.clamp(0, duration) / TICKS_PER_SECOND) as u64,
        (duration / TICKS_PER_SECOND) as u64,
    ))
}
