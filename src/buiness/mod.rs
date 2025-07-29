use flume::{Receiver, Sender};

#[derive(Debug)]
pub enum Business {
    StartAndDetachOneThread,
    SpawnUringInSpecificThread,
}

#[derive(Debug)]
pub enum BusinessOutput {
    StartAndDetachOneThread(std::thread::ThreadId),
}

pub struct BusinessCtx {
    input_sender: Sender<Business>,
    input_receiver: Receiver<Business>,
    output_sender: Sender<BusinessOutput>,
    output_receiver: Receiver<BusinessOutput>,
}

impl BusinessCtx {
    pub fn init<const N: usize>() -> Self {
        let (input_sender, input_receiver) = flume::bounded::<Business>(N);
        let (output_sender, output_receiver) = flume::bounded::<BusinessOutput>(N);
        Self {
            input_sender,
            input_receiver,
            output_sender,
            output_receiver,
        }
    }

    pub fn run(&self, business: &Business) {
        match business {
            Business::StartAndDetachOneThread => {
                let t = std::thread::spawn(|| {});
                let output = BusinessOutput::StartAndDetachOneThread(t.thread().id());
                self.output_sender
                    .send(output)
                    .expect("unreachable, output channel should not be closed");
            }
            Business::SpawnUringInSpecificThread => {
                todo!()
            }
        }
    }
}
