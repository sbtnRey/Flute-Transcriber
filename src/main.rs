//Author: Sebastian Reynolds (sxr@pdx.edu)

extern crate gtk;
extern crate portaudio;
extern crate rusty_microphone;

use gtk::prelude::*;
use std::io::prelude::*;
use portaudio as pa;
use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::sync::mpsc::*;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::fs::OpenOptions;




use rusty_microphone::audio;
use rusty_microphone::model::Model;
use rusty_microphone::signal::Signal;

//-------------------------------------------------------------------------------------------------
// Below code is partially sourced and heavliy modified from https://github.com/JWorthe/rusty_microphone
// specifically from the authors gui.rs file. the repo is open sourced under the MIT license.
//-------------------------------------------------------------------------------------------------
const FPS: u32 = 60;

struct Ui {
    dropdown: gtk::ComboBoxText,
    note_tracker: gtk::Label,
}

struct ApplicationState {
    pa: pa::PortAudio,
    pa_stream: Option<pa::Stream<pa::NonBlocking, pa::Input<f32>>>,
    ui: Ui,
}

// Sets up portaudio devices and calls primary audio functions
pub fn gui() -> Result<(), String> {
    let pa = try!(::audio::init().map_err(|e| e.to_string()));
    let microphones = try!(::audio::get_device_list(&pa).map_err(|e| e.to_string()));
    let default_microphone = try!(::audio::get_default_device(&pa).map_err(|e| e.to_string()));

    try!(gtk::init().map_err(|_| "Failed to initialize GTK."));

    let state = Rc::new(RefCell::new(ApplicationState {
        pa: pa,
        pa_stream: None,
        ui: create_window(microphones, default_microphone),
    }));

    let cross_thread_state = Arc::new(RwLock::new(Model::new()));

    let (mic_sender, mic_receiver) = channel();

    connect_dropdown_choose_microphone(mic_sender, Rc::clone(&state));
    start_processing_audio(mic_receiver, Arc::clone(&cross_thread_state));
    tracker(Rc::clone(&state), Arc::clone(&cross_thread_state));

    gtk::main();
    Ok(())
}

//Creates gui window primariliy to just allow the user to pick which input mic
// if there are multiple
fn create_window(microphones: Vec<(u32, String)>, default_microphone: u32) -> Ui {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Note Tracker");
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    window.add(&vbox);

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    vbox.add(&hbox);
    let dropdown = gtk::ComboBoxText::new();
    dropdown.set_hexpand(true);
    set_dropdown_items(&dropdown, microphones, default_microphone);
    hbox.add(&dropdown);

    let note_tracker = gtk::Label::new(None);
    note_tracker.set_size_request(40, 0);
    vbox.add(&note_tracker);

    window.show_all();

    Ui {
        dropdown: dropdown,
        note_tracker: note_tracker,
    }
}


fn start_processing_audio(mic_receiver: Receiver<Signal>, cross_thread_state: Arc<RwLock<Model>>) {
    thread::spawn(move || {
        while let Ok(signal) = mic_receiver.recv() {
            while mic_receiver.try_recv().is_ok() {}

            let new_model = Model::from_signal(signal);

            match cross_thread_state.write() {
                Ok(mut model) => *model = new_model,
                Err(err) => {
                    println!("Error updating cross thread state: {}", err);
                }
            };
        }
    });
}

//Creates dropdown menu at the top of the gui
fn set_dropdown_items(
    dropdown: &gtk::ComboBoxText,
    microphones: Vec<(u32, String)>,
    default_mic: u32,
) {
    for (index, name) in microphones {
        dropdown.append(Some(format!("{}", index).as_ref()), name.as_ref());
    }
    dropdown.set_active_id(Some(format!("{}", default_mic).as_ref()));
}

// Allows for user to choose mic being used
fn connect_dropdown_choose_microphone(
    mic_sender: Sender<Signal>,
    state: Rc<RefCell<ApplicationState>>,
) {
    let dropdown = state.borrow().ui.dropdown.clone();
    start_listening_current_dropdown_value(&dropdown, mic_sender.clone(), &state);
    dropdown.connect_changed(move |dropdown: &gtk::ComboBoxText| {
        start_listening_current_dropdown_value(dropdown, mic_sender.clone(), &state)
    });
}


fn start_listening_current_dropdown_value(
    dropdown: &gtk::ComboBoxText,
    mic_sender: Sender<Signal>,
    state: &Rc<RefCell<ApplicationState>>,
) {
    if let Some(ref mut stream) = state.borrow_mut().pa_stream {
        stream.stop().ok();
    }
    let selected_mic = match dropdown.get_active_id().and_then(|id| id.parse().ok()) {
        Some(mic) => mic,
        None => {
            return;
        }
    };
    let stream = ::audio::start_listening(&state.borrow().pa, selected_mic, mic_sender).ok();
    if stream.is_none() {
        writeln!(io::stderr(), "Failed to open audio channel").ok();
    }
    state.borrow_mut().pa_stream = stream;
}
//-------------------------------------------------------------------------------------------------

// Tracks and calls pitch strings
fn tracker(state: Rc<RefCell<ApplicationState>>, cross_thread_state: Arc<RwLock<Model>>) {

    let mut pitch_string = "".to_string();
    let mut noise = false;
    let mut prevPitch = Vec::new();

    let f = File::create("transcription.txt").expect("Unable to create file");


    prevPitch.push("".to_string());
    prevPitch.push("".to_string());

    gtk::timeout_add(3000 / FPS, move || {
        let ui = &state.borrow().ui;

        if let Ok(cross_thread_state) = cross_thread_state.read() {
            // Set pitch value
            let mut pitch = &cross_thread_state.pitch_display();

            // convert to string
            pitch_string = pitch.to_string();


            if pitch_string != prevPitch[0] && pitch_string != prevPitch[1] {
                if noise == false{
                    transcription(pitch_string.to_string());
                }



                if pitch == "G 4" || pitch == "G♯"{
                    prevPitch[0] = "G 4".to_string();
                    prevPitch[1] = "G♯4".to_string();
                    noise = false;
                } else if pitch == "A 4" {
                    prevPitch[0] = "A 4".to_string();
                    prevPitch[1] = "A 4".to_string();
                    noise = false;
                } else if pitch == "B♭4" || pitch == "B 4"{
                    prevPitch[0] = "B♭4".to_string();
                    prevPitch[1] = "B 4".to_string();
                    noise = false;
                } else if pitch == "C 5" || pitch == "C♯5"{
                    prevPitch[0] = "C 5".to_string();
                    prevPitch[1] = "C♯5".to_string();
                    noise = false;
                } else if pitch == "D 5"{
                    prevPitch[0] = "D 5".to_string();
                    prevPitch[1] = "D 5".to_string();
                    noise = false;
                } else if pitch == "E♭5"{
                    prevPitch[0] = "E♭5".to_string();
                    prevPitch[1] = "E♭5".to_string();
                    noise = false;
                } else if pitch == "F♯5"{
                    prevPitch[0] = "F♯5".to_string();
                    prevPitch[1] = "F♯5".to_string();
                    noise = false;
                } else if pitch == "G 5"{
                    prevPitch[0] = "G 5".to_string();
                    prevPitch[1] = "G 5".to_string();
                    noise = false;
                }else if pitch == "" {
                    prevPitch[0] = "".to_string();
                    noise = false;
                } else {
                    noise = true;
                }

            }

        }

        gtk::Continue(true)
    });
}

//Ouputs flute keys(6key+HiDo) to terminal and writes to file
fn transcription(pitch: String) {

    let mut f = OpenOptions::new().append(true).open("transcription.txt").unwrap();


    // Currently only Vivaldi Minor Flute
    if pitch == "G 4" || pitch == "G♯4"{
        println!("6");
        let _ = f.write_all(" 6 ".as_bytes());
        f.flush().unwrap();

    }
    if pitch == "A 4"{
        println!("5");
        let _ = f.write_all(" 5 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "B♭4" || pitch == "B 4"{
        println!("4");
        let _ = f.write_all(" 5 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "C 5" || pitch == "C♯5"{
        println!("3");
        let _ = f.write_all(" 3 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "D 5"{
        println!("2");
        let _ = f.write_all(" 2 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "E♭5"{
        println!("1");
        let _ = f.write_all(" 1 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "F♯5"{
        println!("0");
        let _ = f.write_all(" 0 ".as_bytes());
        f.flush().unwrap();
    }
    if pitch == "G 5"{
        println!("HiDo");
        let _ = f.write_all(" HiDo ".as_bytes());
        f.flush().unwrap();
    }

}

fn main() {
    let gui_result = gui();
}
