#[macro_use]
extern crate serde_json;

mod obs;
mod window;

use obs::*;
use std::ffi::OsString;
use std::fs::read_dir;
use std::io::stdin;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};
use window::*;

fn main() {
    println!(
        r"Thanks for using OWReplayRenderer, brought to you by boringcactus.
Before we get started, make sure everything's all ready to go:
- OBS and Overwatch are both running
- OBS has `obs-websocket` installed and running on port 4444 with no authentication
- Overwatch has all the default keybinds for the replay viewer: F1-F12 for player focus, Ctrl+P for pause
- Overwatch has Ctrl+Left bound to 'Jump to Start'
Got all that? Press Enter to continue."
    );
    let _ = read_line();

    let mut obs = OBSClient::new();
    let record_dir = obs.use_subdir();

    record(&mut obs);
    mux(record_dir);
}

fn record(obs: &mut OBSClient) {
    println!(
        r"Sweet, now get your specific replay ready.
- Open the replay, let it load, and pause it
- Check the max timecode the interface shows on the scrub bar (ex. 00:09:31)"
    );
    let replay_length = get_replay_length();
    println!("Which team do you want to see? Red or blue?");
    let side = get_side();
    println!(
        r"That's all we need! You'll need to re-focus Overwatch yourself, so this tool can send it keyboard shortcuts.
It'll render the entire game from each player's perspective, which will take a while.
You can't do anything else with your computer during that time, either, unfortunately.
Once everything is rendered, it'll exit the replay viewer automatically; when that happens, alt+tab back here and
this tool will help you stitch those videos all together for easier viewing."
    );
    println!(
        "Alt-tab back into Overwatch and then come back in *checks notes* {} minutes",
        6 * replay_length.as_secs() / 60
    );
    let overwatch = Window::overwatch();

    overwatch.await_focus();

    for player in side {
        record_once(player, &replay_length, obs, &overwatch);
    }

    small_sleep();
    overwatch.send(&Escape);
    small_sleep();
    overwatch.send(&Up);
    small_sleep();
    overwatch.send(&Up);
    small_sleep();
    overwatch.send(&Space);
    small_sleep();

    println!("Finished recording everyone's perspective!")
}

fn read_line() -> String {
    let stdin = stdin();
    let mut result = String::new();
    stdin.read_line(&mut result).unwrap();
    result.trim().to_string()
}

fn get_replay_length() -> Duration {
    loop {
        println!("Enter the replay length (ex. 00:09:31):");
        let text = read_line();
        let pieces: Vec<&str> = text.split(":").collect();
        let int_pieces: Vec<Result<u64, _>> = pieces.iter().map(|x| x.parse()).collect();
        match int_pieces.as_slice() {
            [Ok(h), Ok(m), Ok(s)] => {
                let m = h * 60 + m;
                let s = m * 60 + s;
                return Duration::from_secs(s);
            },
            _ => println!("Invalid replay length. Enter hours, followed by a colon, followed by minutes, followed by a colon, followed by seconds.")
        }
    }
}

fn get_side() -> Vec<Key> {
    loop {
        println!("Enter \"red\" or \"blue\":");
        let text = read_line();
        match text.to_lowercase().as_str() {
            "blue" => return vec![F1, F2, F3, F4, F5, F6],
            "red" => return vec![F7, F8, F9, F10, F11, F12],
            _ => println!("That's not \"red\" or \"blue\"! Which side do you want to see?"),
        }
    }
}

fn record_once(player: Key, replay_length: &Duration, obs: &mut OBSClient, overwatch: &Window) {
    // make sure we don't start while overwatch is not focused
    overwatch.await_focus();
    // tell overwatch to watch the designated player
    overwatch.send(&player);
    small_sleep();
    // tell overwatch to skip to the beginning
    overwatch.send(&ctrl(Left));
    // tell OBS to start recording
    obs.start_recording();
    // wait a bit so OBS can catch up
    big_sleep();
    // tell overwatch to unpause
    overwatch.send(&ctrl(P));
    small_sleep();
    // can't just wait for the game to end bc if ppl aren't loaded in at the start
    // then we don't have them available to focus. instead, we spam once a second until the game ends
    let game_end = Instant::now() + replay_length.clone();
    while Instant::now() < game_end {
        overwatch.send(&player);
        med_sleep();
    }
    // wait another second
    med_sleep();
    // stop recording
    obs.stop_recording();
    // wait a bit
    sleep(Duration::from_secs(2));
    // jump to beginning again
    overwatch.send(&ctrl(Left));
    med_sleep();
    // re-pause since reaching end doesn't actually pause
    overwatch.send(&ctrl(P));
    println!("Recording following user {:?} finished", player);
}

fn small_sleep() {
    sleep(Duration::from_millis(100));
}

fn med_sleep() {
    sleep(Duration::from_secs(1));
}

fn big_sleep() {
    sleep(Duration::from_secs(2));
}

/// Multiplex all those pieces into a video file with one track for each video,
/// plus one track with a whole matrix overview exclusively for the purpose of flexing.
fn mux(record_dir: PathBuf) {
    if !has_ffmpeg() {
        println!(
            r"This tool can also merge each of those perspectives into one big file containing
each video, so you can switch between perspectives after the fact and keep everything lined up.
Unfortunately, the tool it uses for this, `ffmpeg`, couldn't be found.
If you want this, go download ffmpeg, put `ffmpeg.exe` right next to `OWReplayRender.exe`, and
come back here and press Enter. If you don't need it, just close this program."
        );
        let _ = read_line();
    }
    println!("All the footage has been recorded; now it's time to combine it.");
    let cameras = read_dir(&record_dir)
        .unwrap()
        .filter_map(|x| x.ok())
        .map(|x| x.file_name())
        .filter(|x| x != "final.mkv" && x != "mosaic.mkv")
        .collect::<Vec<_>>();
    let mut inputs = cameras
        .into_iter()
        .flat_map(|x| vec![OsString::from("-i"), x])
        .collect::<Vec<_>>();

    println!("Building mosaic...");
    let filter = r"
        nullsrc=size=1920x720:r=60 [base];
        [0:v] setpts=PTS-STARTPTS, scale=640x360 [upperleft];
        [1:v] setpts=PTS-STARTPTS, scale=640x360 [uppermiddle];
        [2:v] setpts=PTS-STARTPTS, scale=640x360 [upperright];
        [3:v] setpts=PTS-STARTPTS, scale=640x360 [lowerleft];
        [4:v] setpts=PTS-STARTPTS, scale=640x360 [lowermiddle];
        [5:v] setpts=PTS-STARTPTS, scale=640x360 [lowerright];
        [base][upperleft] overlay=shortest=1 [tmp1];
        [tmp1][uppermiddle] overlay=shortest=1:x=640 [tmp2];
        [tmp2][upperright] overlay=shortest=1:x=1280 [tmp3];
        [tmp3][lowerleft] overlay=shortest=1:y=360 [tmp4];
        [tmp4][lowermiddle] overlay=shortest=1:y=360:x=640 [tmp5];
        [tmp5][lowerright] overlay=shortest=1:y=360:x=1280
    ";
    let result = Command::new("ffmpeg")
        .args(&["-y", "-hide_banner", "-v", "warning", "-stats"])
        .args(&inputs)
        .arg("-filter_complex")
        .arg(filter)
        .args(&[
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-an",
        ])
        .arg("mosaic.mkv")
        .current_dir(&record_dir)
        .status()
        .unwrap();
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }
    inputs.append(&mut vec![
        OsString::from("-i"),
        OsString::from("mosaic.mkv"),
    ]);

    println!("Merging...");
    let out_name = {
        let mut path = PathBuf::from("..");
        path.push(
            record_dir
                .file_name()
                .expect("Failed to get directory name"),
        );
        path.with_extension("mkv")
    };
    let maps = (0..(inputs.len() / 2))
        .flat_map(|x| vec!["-map".to_string(), format!("{}", x)])
        .collect::<Vec<_>>();
    let result = Command::new("ffmpeg")
        .args(&["-y", "-hide_banner", "-v", "warning", "-stats"])
        .args(inputs)
        .args(&[
            "-filter_complex",
            "[0:a][1:a][2:a][3:a][4:a][5:a] amix=inputs=6",
        ])
        .args(maps)
        .args(&["-c:v", "copy", "-c:a", "aac"])
        .arg(&out_name)
        .current_dir(&record_dir)
        .status()
        .unwrap();
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }

    println!(
        r#"Done! Your final video is in your OBS output folder, called "{}".
Playing it with VLC or MPC-HC will let you switch video tracks on the fly, but I recommend mpv, because
you can bind keyboard shortcuts to switching video tracks instead of digging through menus.
Bind 1 to `set vid 1`, 2 to `set vid 2`, ..., and 0 to `set vid 7` to match my setup.
Press Enter to exit this tool. Have a nice day!"#,
        out_name
            .file_name()
            .and_then(|x| x.to_str())
            .expect("Failed to render output file name")
    );
    let _ = read_line();
}

fn has_ffmpeg() -> bool {
    let result = Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    result.success()
}

pub fn timestamp() -> String {
    use chrono::prelude::*;
    let now: DateTime<Local> = Local::now();
    now.format("%Y-%m-%d %H-%M-%S").to_string()
}
