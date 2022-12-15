#![feature(naked_functions)]

use core::arch::asm;

const SSIZE: isize = 1024 * 1024 * 2; // 2M
static mut RUNTIME: *mut Runtime = std::ptr::null_mut();

#[derive(Debug, Default)]
#[repr(C)]
struct ThreadContext {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
}

enum State {
    Available,
    Ready,
    Runing,
}

struct GreenThread {
    id: u64,
    stack: Box<[u8]>,
    state: State,
    ctx: ThreadContext,
}

struct Runtime {
    current: u64,
    threads: Vec<GreenThread>,
}

impl Runtime {
    const MAX_NUM_THREADS: usize = 4;

    fn new() -> Self {
        let main_thread = GreenThread {
            id: 0,
            stack: Box::from([0_u8; SSIZE as usize]),
            state: State::Runing,
            ctx: ThreadContext::default(),
        };

        let other_threads = (0..Self::MAX_NUM_THREADS).map(|i| GreenThread {
            id: i as u64,
            stack: Box::from([0_u8; SSIZE as usize]),
            state: State::Available,
            ctx: ThreadContext::default(),
        });

        let mut threads = vec![main_thread];
        threads.extend(other_threads);

        Runtime {
            current: 0,
            threads,
        }
    }

    fn init(&mut self) {
        unsafe {
            if RUNTIME.is_null() {
                RUNTIME = self as *mut Runtime;
            }
        }
    }

    fn run(&mut self) {
        while self.t_yeild() {}
        std::process::exit(0);
    }

    fn t_return(&mut self) {
        if self.current != 0 {
            self.threads[self.current as usize].state = State::Available;
            self.t_yeild();
        }
    }

    #[inline(never)]
    fn t_yeild(&mut self) -> bool {
        let mut pos = (self.current + 1) % (self.threads.len() as u64);
        loop {
            match self.threads[pos as usize].state {
                State::Ready => {
                    break;
                }
                _ if pos == self.current => {
                    return false;
                }
                _ => {
                    pos = (pos + 1) % (self.threads.len() as u64);
                }
            };
        }

        match self.threads[self.current as usize].state {
            State::Available => {}
            _ => self.threads[self.current as usize].state = State::Ready,
        }

        self.threads[pos as usize].state = State::Runing;
        let old_pos = self.current as usize;
        self.current = pos;

        unsafe {
            let old: *mut ThreadContext = &mut self.threads[old_pos].ctx;
            let new: *const ThreadContext = &self.threads[pos as usize].ctx;
            asm!("call switch", in("rdi") old, in("rsi") new, clobber_abi("C"))
        }

        self.threads.len() > 0
    }

    fn spawn(&mut self, f: fn()) {
        let avaiable = self
            .threads
            .iter_mut()
            .find(|t| match t.state {
                State::Available => true,
                _ => false,
            })
            .expect("should find a available green thread");

        unsafe {
            let stack_ptr = avaiable.stack.as_mut_ptr();
            let stack_bottom =
                (stack_ptr.offset(avaiable.stack.len() as isize) as u64 & !15) as *mut u8;
            std::ptr::write(stack_bottom.offset(-16) as *mut u64, guard as u64);
            std::ptr::write(stack_bottom.offset(-24) as *mut u64, skip as u64);
            std::ptr::write(stack_bottom.offset(-32) as *mut u64, f as u64);
            avaiable.ctx.rsp = stack_bottom.offset(-32) as u64;
        }

        avaiable.state = State::Ready;
    }
}

#[naked]
unsafe extern "C" fn skip() {
    asm!("ret", options(noreturn));
}

fn guard() {
    unsafe {
        (*RUNTIME).t_return();
    }
}

fn yeild_thread() {
    unsafe {
        (*RUNTIME).t_yeild();
    }
}

#[naked]
#[no_mangle]
unsafe extern "C" fn switch() {
    unsafe {
        asm!(
            "mov [rdi + 0x00], rsp",
            "mov [rdi + 0x08], r15",
            "mov [rdi + 0x10], r14",
            "mov [rdi + 0x18], r13",
            "mov [rdi + 0x20], r12",
            "mov [rdi + 0x28], rbx",
            "mov [rdi + 0x30], rbp",
            "mov rsp, [rsi + 0x00]",
            "mov r15, [rsi + 0x08]",
            "mov r14, [rsi + 0x10]",
            "mov r13, [rsi + 0x18]",
            "mov r12, [rsi + 0x20]",
            "mov rbx, [rsi + 0x28]",
            "mov rbp, [rsi + 0x30]",
            "ret",
            options(noreturn)
        );
    }
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.init();

    runtime.spawn(|| {
        println!("THREAD 1 Start");
        for i in 0..10 {
            println!("THREAD 1: {i}");
            yeild_thread();
        }
        println!("THREAD 1 finish");
    });

    runtime.spawn(|| {
        println!("THREAD 2 Start");
        for i in 0..15 {
            println!("THREAD 2: {i}");
            yeild_thread();
        }
        println!("THREAD 2 finish");
    });
    runtime.run();
}
