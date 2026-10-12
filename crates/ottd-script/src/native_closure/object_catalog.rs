//! Closure, generator, thread, class, instance and weak-reference registrations.
use super::{DefaultFunction as D, NativeFunction as F, NativeRegistration as R};
pub(super) const CLOSURE: &[R] = &[
    R::new(b"call", F::Default(D::ClosureCall), -1, Some("c"), &[768]),
    R::new(b"pcall", F::Default(D::ClosurePcall), -1, Some("c"), &[768]),
    R::new(
        b"acall",
        F::Default(D::ClosureAcall),
        2,
        Some("ca"),
        &[768, 64],
    ),
    R::new(
        b"pacall",
        F::Default(D::ClosurePacall),
        2,
        Some("ca"),
        &[768, 64],
    ),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
    R::new(
        b"bindenv",
        F::Default(D::ClosureBindenv),
        2,
        Some("c x|y|t"),
        &[768, 49184],
    ),
    R::new(
        b"getinfos",
        F::Default(D::ClosureGetinfos),
        1,
        Some("c"),
        &[768],
    ),
];
pub(super) const GENERATOR: &[R] = &[
    R::new(
        b"getstatus",
        F::Default(D::GeneratorGetstatus),
        1,
        Some("g"),
        &[1024],
    ),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
];
pub(super) const THREAD: &[R] = &[
    R::new(b"call", F::Default(D::ThreadCall), -1, Some("v"), &[4096]),
    R::new(
        b"wakeup",
        F::Default(D::ThreadWakeup),
        -1,
        Some("v"),
        &[4096],
    ),
    R::new(
        b"getstatus",
        F::Default(D::ThreadGetstatus),
        1,
        Some("v"),
        &[4096],
    ),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
];
pub(super) const CLASS: &[R] = &[
    R::new(
        b"getattributes",
        F::Default(D::ClassGetattributes),
        2,
        Some("y."),
        &[16384, -1],
    ),
    R::new(
        b"setattributes",
        F::Default(D::ClassSetattributes),
        3,
        Some("y.."),
        &[16384, -1, -1],
    ),
    R::new(
        b"rawin",
        F::Default(D::ContainerRawexists),
        2,
        Some("y"),
        &[16384],
    ),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
    R::new(
        b"instance",
        F::Default(D::ClassInstance),
        1,
        Some("y"),
        &[16384],
    ),
];
pub(super) const INSTANCE: &[R] = &[
    R::new(
        b"getclass",
        F::Default(D::InstanceGetclass),
        1,
        Some("x"),
        &[32768],
    ),
    R::new(
        b"rawin",
        F::Default(D::ContainerRawexists),
        2,
        Some("x"),
        &[32768],
    ),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
];
pub(super) const WEAKREF: &[R] = &[
    R::new(b"ref", F::Default(D::WeakrefRef), 1, Some("r"), &[65536]),
    R::new(b"weakref", F::Default(D::ObjDelegateWeakref), 1, None, &[]),
    R::new(
        b"tostring",
        F::Default(D::DefaultDelegateTostring),
        1,
        Some("."),
        &[-1],
    ),
];
