//Author: Sebastian Reynolds (sxr@pdx.edu)

extern crate rusty_microphone;
extern crate gtk;
extern crate portaudio;


use gtk::prelude::*;
use std::cell::RefCell;
use portaudio as pa;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;
use std::io;
use std::io::Write;
use std::thread;
use std::sync::mpsc::*;

use rusty_microphone::model::Model;
use rusty_microphone::audio;
use rusty_microphone::signal::Signal;




//-------------------------------------------------------------------------------------------------
// Below code is partially sourced and heavliy modified from https://github.com/JWorthe/rusty_microphone
//-------------------------------------------------------------------------------------------------
const FPS: u32 = 60;

struct Ui {
    dropdown: gtk::ComboBoxText,
    note_tracker: gtk::Label,
}

struct ApplicationState {
    pa: pa::PortAudio,
    pa_stream: Option<pa::Stream<pa::NonBlocking, pa::Input<f32>>>,
    ui: Ui
}

pub fn gui() -> Result<(), String> {
    let pa = try!(::audio::init().map_err(|e| e.to_string()));
    let microphones = try!(::audio::get_device_list(&pa).map_err(|e| e.to_string()));
    let default_microphone = try!(::audio::get_default_device(&pa).map_err(|e| e.to_string()));

    try!(gtk::init().map_err(|_| "Failed to initialize GTK."));

    let state = Rc::new(RefCell::new(ApplicationState {
        pa: pa,
        pa_stream: None,
        ui: create_window(microphones, default_microphone)
    }));

    let cross_thread_state = Arc::new(RwLock::new(Model::new()));

    let (mic_sender, mic_receiver) = channel();


    connect_dropdown_choose_microphone(mic_sender, Rc::clone(&state));
    start_processing_audio(mic_receiver, Arc::clone(&cross_thread_state));
    tracker(Rc::clone(&state), Arc::clone(&cross_thread_state));

    gtk::main();
    Ok(())
}
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
                Ok(mut model) => {
                    *model = new_model
                },
                Err(err) => {
                    println!("Error updating cross thread state: {}", err);
                }
            };
        }
    });
}

fn set_dropdown_items(dropdown: &gtk::ComboBoxText, microphones: Vec<(u32, String)>, default_mic: u32) {
    for (index, name) in microphones {
        dropdown.append(Some(format!("{}", index).as_ref()), name.as_ref());
    }
    dropdown.set_active_id(Some(format!("{}", default_mic).as_ref()));
}

fn connect_dropdown_choose_microphone(mic_sender: Sender<Signal>, state: Rc<RefCell<ApplicationState>>) {
    let dropdown = state.borrow().ui.dropdown.clone();
    start_listening_current_dropdown_value(&dropdown, mic_sender.clone(), &state);
    dropdown.connect_changed(move |dropdown: &gtk::ComboBoxText| {
        start_listening_current_dropdown_value(dropdown, mic_sender.clone(), &state)
    });
}

fn start_listening_current_dropdown_value(dropdown: &gtk::ComboBoxText, mic_sender: Sender<Signal>, state: &Rc<RefCell<ApplicationState>>) {
    if let Some(ref mut stream) = state.borrow_mut().pa_stream {
        stream.stop().ok();
    }
    let selected_mic = match dropdown.get_active_id().and_then(|id| id.parse().ok()) {
        Some(mic) => mic,
        None => {return;}
    };
    let stream = ::audio::start_listening(&state.borrow().pa, selected_mic, mic_sender).ok();
    if stream.is_none() {
        writeln!(io::stderr(), "Failed to open audio channel").ok();
    }
    state.borrow_mut().pa_stream = stream;
}
//-------------------------------------------------------------------------------------------------


fn tracker(state: Rc<RefCell<ApplicationState>>, cross_thread_state: Arc<RwLock<Model>>) {

    let mut test_string = "".to_string();
    gtk::timeout_add(1000/FPS, move || {
        let ui = &state.borrow().ui;

        if let Ok(cross_thread_state) = cross_thread_state.read() {
            let mut pitch = &cross_thread_state.pitch_display();
            let mut track = pitch;


            test_string = pitch.to_string();
            if pitch != "" || pitch.to_string() != test_string {

                transcription(test_string.to_string());
            }

        }

        gtk::Continue(true)
    });
}


fn transcription(pitch:String){
    println!("{}", pitch)
}

fn main() {

    let gui_result = gui();

}
