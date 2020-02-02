use crate::cycles;
use crate::gb::{GBReason, GB};
use crate::peripherals::PeripheralData;
use bincode;
use std::collections::VecDeque;

const MAX_LEN: usize = 60 * 60;
//const MAX_LEN: usize = 60;

struct State {
    s: Vec<u8>,
}

pub struct Rewind {
    gb: GB,
    states: VecDeque<State>,
    free: Vec<Vec<u8>>,
    pos: Option<usize>,
}

impl Rewind {
    pub fn gb(&mut self) -> &mut GB {
        &mut self.gb
    }
    pub fn step(
        &mut self,
        time: Option<cycles::CycleCount>,
        real: &mut PeripheralData,
    ) -> GBReason {
        self.cont();
        let r = self.gb.step(time, real);
        match r {
            GBReason::VSync => self.snapshot(),
            _ => {}
        }
        r
    }

    pub fn new(gb: GB) -> Rewind {
        Rewind {
            gb,
            states: VecDeque::new(),
            free: Vec::new(),
            pos: None,
        }
    }
    fn snapshot(&mut self) {
        let mut patch = if let Some(v) = self.free.pop() {
            v
        } else if self.states.len() > MAX_LEN {
            self.pos = self.pos.map(|p| p.saturating_sub(1));
            self.states.pop_front().unwrap().s
        } else {
            Vec::new()
        };
        patch.clear();
        let patch = bincode::serialize(&self.gb).unwrap();
        self.states.push_back(State { s: patch });
        self.pos = if let Some(i) = self.pos {
            Some(i + 1)
        } else {
            Some(0)
        }
    }
    pub fn restore(&mut self, data: &mut PeripheralData) {
        if let Some(pos) = self.pos {
            self.gb =
                bincode::deserialize(&self.states[pos].s).expect("Invalid Bincode Deserialize");
        }

        loop {
            if let GBReason::VSync = self.gb.step(None, data) {
                break;
            }
        }
    }
    pub fn back(&mut self) {
        self.pos = self.pos.map(|pos| pos.saturating_sub(1));
    }
    pub fn forward(&mut self) {
        self.pos = self
            .pos
            .map(|pos| std::cmp::min(self.states.len().saturating_sub(1), pos.saturating_add(1)));
    }
    fn cont(&mut self) {
        if let Some(pos) = self.pos {
            if pos < self.states.len() - 1 {
                self.free
                    .extend(self.states.drain(pos + 1..).map(|state| state.s))
            }
        }
    }
}
