//! Source-mirroring scalar state of one materialized context.
use crate::heap::Stored;
/// Source-default scalar state of one allocated but inactive `CallInfo`
/// record; the current observer exposes no experimental credit for these
/// values until active frames bind them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CallScalars {
    pub(crate) root: bool,
    pub(crate) etraps: i32,
    pub(crate) prevbase: i32,
    pub(crate) prevtop: i32,
    pub(crate) target: i32,
    pub(crate) ncalls: i32,
    pub(crate) varg_base: u16,
    pub(crate) varg_size: u16,
}
impl CallScalars {
    const fn allocated() -> Self {
        Self {
            root: false,
            etraps: 0,
            prevbase: 0,
            prevtop: 0,
            target: 0,
            ncalls: 0,
            varg_base: 0,
            varg_size: 0,
        }
    }
}
/// One allocated native `CallInfo` record.
#[derive(Debug)]
pub(crate) struct CallRecord {
    pub(crate) closure: Stored,
    pub(crate) scalars: CallScalars,
}
impl CallRecord {
    pub(crate) const fn allocated() -> Self {
        Self {
            closure: Stored::null(),
            scalars: CallScalars::allocated(),
        }
    }
}
/// Scalar counter copy used by idle observation; widths mirror the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "The source SQVM member state mirrors its exact SQBool fields"
)]
pub(crate) struct ContextScalars {
    pub(crate) suspended: bool,
    pub(crate) suspended_target: i64,
    pub(crate) suspended_root: bool,
    pub(crate) suspended_traps: i32,
    pub(crate) nnativecalls: i32,
    pub(crate) can_suspend: bool,
    pub(crate) in_stackoverflow: bool,
    pub(crate) ops_till_suspend: i64,
    pub(crate) ops_till_suspend_error_threshold: i64,
    pub(crate) top: i64,
    pub(crate) stackbase: i64,
    pub(crate) callsstacksize: i64,
    pub(crate) etraps: i64,
    pub(crate) stack_size: usize,
    pub(crate) vargs_size: usize,
    pub(crate) allocated_calls: usize,
}
