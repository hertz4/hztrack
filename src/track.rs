use std::sync::Arc;

use mixer::*;

use base32;

pub struct Track {
    seq:        Vec<Vec<Field>>,
    row:        usize,
    tick_count: u8,
    bpm:        u8,
    tick_rate:  u8,
    row_jump:   Option<usize>,
    pcm:        Arc<Vec<i8>>,
    output:     MixerIn,
}

#[derive(Clone)]
pub struct Field {
    pub note:   Note,
    pub cmd:    Command,
}

#[derive(Clone)]
pub enum Note {
    On(u8),
    Off,
    Hold,
}

#[derive(Clone)]
pub struct Command {
    pub id:     u8,
    pub data:   u8,
}

impl Track {
    pub fn new(seq: Vec<Vec<Field>>) -> Self {
        Track {
            seq: seq,
            row: 0,
            tick_count: 0,
            bpm: 120,
            tick_rate: 6,
            row_jump: None,
            pcm: Arc::new((0..256)
                .map(|i| ((i as f64 / 128.0 * 3.1415).sin() * 127.0) as i8)
                .collect()),
            output: MixerIn::new(),
        }
    }
}

impl Controller for Track {
    fn next(&mut self) -> MixerIn {
        const DEFAULT_CHAN: ChannelIn = ChannelIn {
            base_note:  60,
            note_off:   0,
            pcm_off:    0,
            pcm_len:    256,
            pcm_rate:   256,
            vol:        0,
        };

        self.output.chan.resize(self.seq[self.row].len(), DEFAULT_CHAN);
        self.tick();
        MixerIn {
            tick_rate: self.bpm as u16 * self.tick_rate as u16,
            pcm: self.pcm.clone(),
            chan: self.output.chan.clone(),
        }
    }
}

impl Track {
    pub fn width(&self) -> usize { self.seq[self.row].len() }
    pub fn row(&mut self) -> &mut Vec<Field> { &mut self.seq[self.row] }
    fn tick(&mut self) {
        self.tick_count += 1;
        for i in 0..self.width() {
            self.channel_tick(i)
        }
        if self.tick_count > self.tick_rate {
            self.beat();
        }
    }
    fn beat(&mut self) {
        self.tick_count = 0;
        self.row = self.row_jump.unwrap_or(
            (self.row + 1) % self.seq.len());
        self.row_jump = None;
        for i in 0..self.width() {
            self.channel_beat(i);
        }
    }
    fn channel_tick(&mut self, i: usize) {
        let chan = &mut self.output.chan[i];
        let field = &self.seq[self.row][i];
        match field.cmd.id as char {
            '0' => {
                chan.note_off = match self.tick_count % 3 {
                    0 => 0,
                    1 => (field.cmd.hi() as i16)<<8,
                    2 => (field.cmd.lo() as i16)<<8,
                    _ => unreachable!(),
                }
            }
            '1' => chan.note_off = match chan.note_off
                .checked_add((field.cmd.data as i16)<<4) {
                    Some(note) => note,
                    None => i16::max_value(),
            },
            '2' => chan.note_off = match chan.note_off
                .checked_sub((field.cmd.data as i16)<<4) {
                    Some(note) => note,
                    None => i16::min_value(),
            },
            'F' => {
                if field.cmd.data < 32 {
                    self.tick_rate = field.cmd.data + 1
                } else {
                    self.bpm = field.cmd.data
                }
            }
            'B' => self.row_jump = Some(field.cmd.data as usize),
            c @ _ => eprintln!("unknown command id: {}", c),
        }
    }
    fn channel_beat(&mut self, i: usize) {
        let chan = &mut self.output.chan[i];
        match self.seq[self.row][i].note {
            Note::On(n) => {
                chan.set_note(n);
                chan.vol = 64;
            }
            Note::Off => chan.vol = 0,
            Note::Hold => {},
        }
    }
}

impl Field {
    pub fn string(&self) -> String {
        let mut out = String::new();
        match self.note {
            Note::On(ref note) => {
                const NOTE_NAME: &'static str = "C-C#D-D#E-F-F#G-G#A-A#B-";
                let name = *note as usize % 12;
                out.push_str(&NOTE_NAME[name*2..name*2+2]);
                let octave = note / 12;
                out.push_str(&format!("{}", octave));
            }
            Note::Off => out.push_str("---"),
            Note::Hold => out.push_str("   "),
        }
        out.push_str(&format!("{}{:02X}", self.cmd.id as char, self.cmd.data));
        out
    }
}

impl ::std::ops::Add<u8> for Note {
    type Output = Note;
    fn add(self, with: u8) -> Note {
        match self {
            Note::On(v) => Note::On(v + with),
            _ => self
        }
    }
}
impl Command {
    pub fn zero() -> Command { Command { id: '0' as u8, data: 0 } }
    pub fn from_str(raw: &str) -> Command {
        let mut chars = raw.chars();
        Command {
            id: base32::from_char(chars.next().unwrap()),
            data: u8::from_str_radix(chars.as_str(), 16).unwrap(),
        }
    }
    pub fn hi(&self) -> u8 { self.data >> 4 }
    pub fn lo(&self) -> u8 { self.data & 0xf }
    pub fn set_hi(&mut self, v: u8) { self.data = self.lo() + (v << 4) }
    pub fn set_lo(&mut self, v: u8) { self.data = (self.data & 0xf0) + (v & 0xf) }
}
