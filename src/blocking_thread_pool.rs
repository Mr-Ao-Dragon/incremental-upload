use std::cell::Cell;
use std::cell::UnsafeCell;
use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::thread::JoinHandle;

type Task = Box<dyn (FnOnce() -> Result<(), Box<dyn Error + Send>>) + Send>;

pub enum WorkerMessage {
    Task(Task),
    Terminate,
}

pub struct Worker {
    pub id: u32,
    pub sender: SyncSender<WorkerMessage>,
    pub receiver: Arc<Mutex<Receiver<WorkerMessage>>>,
    pub thread: Option<JoinHandle<()>>,
    pub exited_flag: bool,
    pub busy: bool,
    pub on_error: Box<dyn Fn(Box<dyn std::error::Error + Send>) + Send>,
}

impl Worker {
    pub fn new(
        id: u32, 
        sender: SyncSender<WorkerMessage>, 
        receiver: Arc<Mutex<Receiver<WorkerMessage>>>,
        on_error: Box<dyn Fn(Box<dyn std::error::Error + Send>) + Send>,
    ) -> Arc<UnsafeCell<Worker>> {
        let worker = Arc::new(UnsafeCell::new(Worker {
            id,
            sender,
            receiver,
            exited_flag: false,
            thread: None,
            busy: false,
            on_error,
        }));

        unsafe {
            let worker_ = &mut *worker.get();
            let worker__ = &mut *worker.get();
            worker_.thread = Option::Some(thread::spawn(move || worker__.run()));
        }
        
        worker
    }

    pub fn run(&mut self) {
        while !self.exited_flag {
            let msg = self.receiver.lock().unwrap().recv().unwrap();
            
            match msg {
                WorkerMessage::Task(task) => {
                    self.busy = true;
                    let result = task();
                    if result.is_err() {
                        (self.on_error)(result.err().unwrap());
                        return;
                    }
                    self.busy = false;
                }

                WorkerMessage::Terminate => {
                    break;
                }
            }
        }
    }

    pub fn is_terminated(&self) -> bool {
        match &self.thread {
            Some(t) => t.is_finished(),
            None => false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn wait(&mut self) {
        if self.thread.is_some() {
            self.thread.take().unwrap().join().unwrap();
        }
    }
}

pub struct BlockingThreadPool {
    workers: Vec<Arc<UnsafeCell<Worker>>>,
    sender: mpsc::SyncSender<WorkerMessage>,
    is_terminated: bool,
    error: Arc<Mutex<Cell<Option<Box<dyn std::error::Error + Send>>>>>
}

impl BlockingThreadPool {
    pub fn new(size: usize) -> BlockingThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::sync_channel(0);
        let workers = Vec::with_capacity(size);
        let receiver = Arc::new(Mutex::new(receiver));

        let mut ins = BlockingThreadPool { workers, sender, is_terminated: false, error: Arc::new(Mutex::new(Cell::new(None))) };
        
        for id in 0..size {
            let ins_copy = ins.error.clone();
            ins.workers.push(Worker::new(id as u32, ins.sender.clone(), receiver.clone(), Box::new(move |err| {
                *ins_copy.lock().unwrap().get_mut() = Some(err);
            })));
        }
        
        ins
    }

    pub fn execute<F>(&self, fun: F) where F : (FnOnce() -> Result<(), Box<dyn Error + Send>>) + Send + 'static, {
        if self.is_terminated {
            panic!("dispatching task after thread pool was closed.");
        }

        self.sender.send(WorkerMessage::Task(Box::new(fun))).unwrap();
    }

    pub fn close_and_wait(&mut self) -> Result<(), Box<dyn Error + Send>> {
        if self.is_terminated {
            return Ok(());
        }

        self.is_terminated = true;

        for _worker in &self.workers {
            self.sender.send(WorkerMessage::Terminate).unwrap();
        }

        for worker in &self.workers {
            unsafe {
                let worker = &mut *worker.get();
                worker.wait();
            }
        }
        
        let err = self.error.lock().unwrap().take();

        err.map_or_else(|| Ok(()), |e| Err(e))
    }

    pub fn size(&self) -> u32 {
        self.workers.len() as u32
    }
}

impl Drop for BlockingThreadPool {
    fn drop(&mut self) {
        if !self.is_terminated {
            self.close_and_wait().unwrap();
        }
    }
}