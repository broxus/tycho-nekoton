mod executor;

use anyhow::Result;
use everscale_types::abi::{AbiValue, Function};
use nekoton_utils::time::{Clock, SimpleClock};

#[derive(Copy, Clone)]
pub struct ExecutionContext<'a> {
    pub clock: &'a dyn Clock,
}

impl<'a> ExecutionContext<'a> {
    pub fn run_local(&self, function: &Function, values: &[AbiValue]) -> Result<()> {
        self.run(function, values, false)
    }

    pub fn run_local_responsible(&self, function: &Function, values: &[AbiValue]) -> Result<()> {
        self.run(function, values, true)
    }

    fn run(&self, function: &Function, values: &[AbiValue], responsible: bool) -> Result<()> {}
}

pub struct ExecutionContextBuilder<'a> {
    pub clock: Option<&'a dyn Clock>,
}

impl ExecutionContextBuilder {
    pub fn new() -> ExecutionContextBuilder {
        Self {
            clock: None,
        }
    }

    pub fn with_clock(mut self, clock: &dyn Clock) -> Self {
        self.clock = Some(clock);
        Self
    }


    pub fn build(&self) -> ExecutionContext {
        ExecutionContext {
            clock: self.clock.unwrap_or(&SimpleClock),
        }
    }
}