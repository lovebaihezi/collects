use flume::{Receiver, Sender};

#[derive(Debug)]
pub enum Business {
    StartAndDetachOneThread,
    SpawnTokioInOneThread,
}

#[derive(Debug)]
pub enum BusinessOutput {
    Void,
}

pub struct BusinessCtx {
    input_sender: Sender<Business>,
    input_receiver: Receiver<Business>,
    output_sender: Sender<BusinessOutput>,
    output_receiver: Receiver<BusinessOutput>,
}

impl BusinessCtx {
    pub fn init<const N: usize>() -> BusinessCtx {
        // TODO: Max Business output should based on
        let (input_sender, input_receiver) = flume::bounded::<Business>(N);
        let (output_sender, output_receiver) = flume::bounded::<BusinessOutput>(N);
        Self {
            input_sender,
            input_receiver,
            output_sender,
            output_receiver,
        }
    }

    pub fn schedule_one_business(business: Business) {}
}
