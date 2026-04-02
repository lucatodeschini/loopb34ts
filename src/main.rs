use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioBuffer, GainNode};
use yew::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

const STEPS: usize = 16;

#[derive(Clone, PartialEq)]
struct DrumSound {
    id: &'static str,
    name: &'static str,
    emoji: &'static str,
    color: &'static str,
}

const DRUM_SOUNDS: [DrumSound; 6] = [
    DrumSound { id: "kick", name: "Kick", emoji: "🥁", color: "#ff6b6b" },
    DrumSound { id: "snare", name: "Snare", emoji: "🪘", color: "#4ecdc4" },
    DrumSound { id: "hihat", name: "Hi-Hat", emoji: "🎩", color: "#ffe66d" },
    DrumSound { id: "clap", name: "Clap", emoji: "👏", color: "#95e1d3" },
    DrumSound { id: "tom", name: "Tom", emoji: "🔊", color: "#c9b1ff" },
    DrumSound { id: "crash", name: "Crash", emoji: "💥", color: "#f38181" },
];

struct AudioEngine {
    context: AudioContext,
    master_gain: GainNode,
    buffers: HashMap<&'static str, AudioBuffer>,
}

impl AudioEngine {
    async fn new() -> Result<Self, JsValue> {
        let context = AudioContext::new()?;
        let master_gain = context.create_gain()?;
        master_gain.gain().set_value(0.7);
        master_gain.connect_with_audio_node(&context.destination())?;
        
        let mut buffers = HashMap::new();
        
        for sound in DRUM_SOUNDS.iter() {
            let url = format!("/samples/{}.wav", sound.id);
            if let Ok(buffer) = Self::load_sample(&context, &url).await {
                buffers.insert(sound.id, buffer);
            }
        }
        
        Ok(Self { context, master_gain, buffers })
    }
    
    async fn load_sample(context: &AudioContext, url: &str) -> Result<AudioBuffer, JsValue> {
        let window = web_sys::window().unwrap();
        let response = JsFuture::from(window.fetch_with_str(url)).await?;
        let response: web_sys::Response = response.dyn_into()?;
        let array_buffer = JsFuture::from(response.array_buffer()?).await?;
        let array_buffer: js_sys::ArrayBuffer = array_buffer.dyn_into()?;
        let audio_buffer = JsFuture::from(context.decode_audio_data(&array_buffer)?).await?;
        Ok(audio_buffer.dyn_into()?)
    }
    
    fn play(&self, sound_id: &str) {
        if let Some(buffer) = self.buffers.get(sound_id) {
            if let Ok(source) = self.context.create_buffer_source() {
                source.set_buffer(Some(buffer));
                source.connect_with_audio_node(&self.master_gain).unwrap();
                source.start().unwrap();
            }
        }
    }
    
    fn resume(&self) -> Result<(), JsValue> {
        let state = js_sys::Reflect::get(&self.context, &js_sys::JsString::from("state"))?
            .as_string()
            .unwrap_or_default();
        if state == "suspended" {
            let _ = JsFuture::from(self.context.resume()?);
        }
        Ok(())
    }
}

#[function_component(App)]
fn app() -> Html {
    let grid = use_state(|| {
        let mut g = HashMap::new();
        for sound in DRUM_SOUNDS.iter() {
            g.insert(sound.id, vec![false; STEPS]);
        }
        g
    });
    
    let is_playing = use_state(|| false);
    let current_step = use_state(|| 0usize);
    let bpm = use_state(|| 120u32);
    let audio = use_state(|| Option::<Rc<AudioEngine>>::None);
    
    // Play interval
    {
        let is_playing = is_playing.clone();
        let current_step = current_step.clone();
        let grid = grid.clone();
        let audio = audio.clone();
        let bpm_val = *bpm;
        
        use_effect(move || {
            if *is_playing {
                let step_duration = (60.0 / bpm_val as f64 * 1000.0 / 4.0) as u32;
                let handle = gloo::timers::callback::Interval::new(step_duration, move || {
                    if let Some(audio_engine) = audio.as_ref() {
                        for sound in DRUM_SOUNDS.iter() {
                            if let Some(row) = grid.get(sound.id) {
                                if row[*current_step] {
                                    audio_engine.play(sound.id);
                                }
                            }
                        }
                    }
                    current_step.set((*current_step + 1) % STEPS);
                });
                Box::new(move || drop(handle)) as Box<dyn FnOnce()>
            } else {
                Box::new(|| {}) as Box<dyn FnOnce()>
            }
        });
    }
    
    let on_play = {
        let is_playing = is_playing.clone();
        let audio = audio.clone();
        Callback::from(move |_| {
            if audio.is_none() {
                let audio_clone = audio.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(engine) = AudioEngine::new().await {
                        engine.resume().ok();
                        audio_clone.set(Some(Rc::new(engine)));
                    }
                });
            }
            is_playing.set(!*is_playing);
        })
    };
    
    let on_clear = {
        let grid = grid.clone();
        let is_playing = is_playing.clone();
        Callback::from(move |_| {
            grid.set({
                let mut new_grid = (*grid).clone();
                for row in new_grid.values_mut() {
                    *row = vec![false; STEPS];
                }
                new_grid
            });
            is_playing.set(false);
        })
    };
    
    let play_text = if *is_playing { "⏸ Stop" } else { "▶ Play" };
    let play_class = if *is_playing { "play-btn playing" } else { "play-btn" };
    
    html! {
        <div class="sequencer">
            <h1 class="title">
                <span class="title-icon">{"🎵"}</span>
                {"LoopB34ts"}
            </h1>
            
            <div class="controls">
                <button class={play_class} onclick={on_play}>
                    {play_text}
                </button>
                <button class="clear-btn" onclick={on_clear}>
                    {"Clear"}
                </button>
                <div class="bpm-control">
                    <label>{format!("BPM: {}", *bpm)}</label>
                    <input 
                        type="range"
                        min="60"
                        max="200"
                        value={bpm.to_string()}
                        oninput={{
                            let bpm = bpm.clone();
                            Callback::from(move |e: InputEvent| {
                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                bpm.set(input.value_as_number() as u32);
                            })
                        }}
                    />
                </div>
            </div>
            
            <div class="grid-container">
                <div class="grid">
                    {for DRUM_SOUNDS.iter().map(|sound| {
                        let sound_id = sound.id;
                        let rows = grid.get(sound.id).cloned().unwrap_or_default();
                        let audio_clone = audio.clone();
                        
                        html! {
                            <div class="row">
                                <button 
                                    class="sound-label"
                                    style={format!("--sound-color: {}", sound.color)}
                                    onclick={Callback::from(move |_| {
                                        if let Some(audio_engine) = audio_clone.as_ref() {
                                            audio_engine.play(sound_id);
                                        }
                                    })}
                                >
                                    <span class="sound-emoji">{sound.emoji}</span>
                                    <span class="sound-name">{sound.name}</span>
                                </button>
                                <div class="steps">
                                    {for rows.iter().enumerate().map(|(i, active)| {
                                        let grid_clone = grid.clone();
                                        let sound_id = sound.id;
                                        let color = sound.color;
                                        
                                        html! {
                                            <button
                                                class={classes!(
                                                    "step",
                                                    if *active { Some("active") } else { None },
                                                    if *current_step == i && *is_playing { Some("current") } else { None }
                                                )}
                                                style={format!("--step-color: {}", color)}
                                                onclick={Callback::from(move |_| {
                                                    grid_clone.set({
                                                        let mut new_grid = (*grid_clone).clone();
                                                        if let Some(row) = new_grid.get_mut(sound_id) {
                                                            row[i] = !row[i];
                                                        }
                                                        new_grid
                                                    });
                                                })}
                                            >
                                                { if *active { "●" } else { "" } }
                                            </button>
                                        }
                                    })}
                                </div>
                            </div>
                        }
                    })}
                </div>
            </div>
            
            <p class="hint">{"Click on the grid to add or remove sounds. Click Play to hear your loop!"}</p>
            
            <footer class="footer">
                <a
                    href="https://ko-fi.com/10uc4"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="kofi-btn"
                >
                    <span class="kofi-icon">{"☕"}</span>
                    {"Buy me a coffee"}
                </a>
            </footer>
        </div>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
