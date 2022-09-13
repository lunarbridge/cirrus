use std::sync::Arc;

use tauri::{State, Window, Runtime, Manager};

use cirrus_client_lib::request;
use cirrus_grpc::api::AudioTagRes;

use crate::state::{AppState, self};

// #[tauri::command]
// pub fn set_action_emit_playback_position(
//     state: State<'_, AppState>,
//     action: String,
// ) {
//     match action.as_str() {
//         "start" => state.ui.audio_player.is_playing = true,
//         "stop" => state.ui.audio_player.is_playing = false,
//     }
// }

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackPayload {
    pos: f32,
    remain_buf: f32,
}

#[tauri::command]
pub fn send_playback_position<R: Runtime>(
    window: Window<R>,
    state: State<'_, AppState>,
) {
    let audio_player = state.audio_player.clone();

    std::thread::spawn(move || loop {
        let playback_payload = PlaybackPayload {
            pos: audio_player.get_playback_position(),
            remain_buf: audio_player.get_remain_sample_buffer_sec(),
        };

        if let Err(e) = window.emit("update-playback-pos", playback_payload) {
            println!("{:?}", e);
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    });
}

#[tauri::command]
pub async fn load_audio(
    state: State<'_, AppState>,
    audio_tag_id: String
) -> Result<f32, &'static str> {

    match state.audio_player.add_audio(&audio_tag_id).await {
        Ok(content_length) => return Ok(content_length),
        Err(_) => return Err("tauri-plugin: failed to add audio"),
    }
}

#[tauri::command]
pub fn start_audio(
    state: State<'_, AppState>
) -> Result<(), &'static str> {

    match state.audio_player.play() {
        Ok(())=> return Ok(()),
        Err(_) => return Err("tauri-plugin: failed to play audio"), 
    }
}

#[tauri::command]
pub fn stop_audio(
    state: State<'_, AppState>
) -> Result<(), &'static str> {

    state.audio_player.stop();

    Ok(())
}

#[tauri::command]
pub fn pause_audio(
    state: State<'_, AppState>
) -> Result<(), &'static str> {

    match state.audio_player.pause() {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to pause audio"),
    }
}

#[tauri::command]
pub async fn get_audio_tags(
    items_per_page: u64,
    page: u32,
) -> Result<Vec<AudioTagRes>, &'static str> {
    println!("got get-audio-tags commnad");

    match request::get_audio_tags(items_per_page, page as u64).await {
        Ok(audio_tags) => Ok(audio_tags),
        Err(_) => return Err("failed to get audio tags from server"),
    }
    // let audio_tags = request::get_audio_tags(items_per_page, page as u64).await.unwrap();

    // Ok(audio_tags)
}