//! Pinned C++ callback identities, separate from closure object identity.
/// Callback identities used by the ten native default delegate registrations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultFunction {
    /// Pinned `default_delegate_len` callback.
    DefaultDelegateLen,
    /// Pinned `table_rawget` callback.
    TableRawget,
    /// Pinned `table_rawset` callback.
    TableRawset,
    /// Pinned `table_rawdelete` callback.
    TableRawdelete,
    /// Pinned `container_rawexists` callback.
    ContainerRawexists,
    /// Pinned `obj_delegate_weakref` callback.
    ObjDelegateWeakref,
    /// Pinned `default_delegate_tostring` callback.
    DefaultDelegateTostring,
    /// Pinned `obj_clear` callback.
    ObjClear,
    /// Pinned `array_append` callback.
    ArrayAppend,
    /// Pinned `array_extend` callback.
    ArrayExtend,
    /// Pinned `array_pop` callback.
    ArrayPop,
    /// Pinned `array_top` callback.
    ArrayTop,
    /// Pinned `array_insert` callback.
    ArrayInsert,
    /// Pinned `array_remove` callback.
    ArrayRemove,
    /// Pinned `array_resize` callback.
    ArrayResize,
    /// Pinned `array_reverse` callback.
    ArrayReverse,
    /// Pinned `array_sort` callback.
    ArraySort,
    /// Pinned `array_slice` callback.
    ArraySlice,
    /// Pinned `default_delegate_tointeger` callback.
    DefaultDelegateTointeger,
    /// Pinned `default_delegate_tofloat` callback.
    DefaultDelegateTofloat,
    /// Pinned `string_slice` callback.
    StringSlice,
    /// Pinned `string_find` callback.
    StringFind,
    /// Pinned `string_tolower` callback.
    StringTolower,
    /// Pinned `string_toupper` callback.
    StringToupper,
    /// Pinned `number_delegate_tochar` callback.
    NumberDelegateTochar,
    /// Pinned `closure_call` callback.
    ClosureCall,
    /// Pinned `closure_pcall` callback.
    ClosurePcall,
    /// Pinned `closure_acall` callback.
    ClosureAcall,
    /// Pinned `closure_pacall` callback.
    ClosurePacall,
    /// Pinned `closure_bindenv` callback.
    ClosureBindenv,
    /// Pinned `closure_getinfos` callback.
    ClosureGetinfos,
    /// Pinned `generator_getstatus` callback.
    GeneratorGetstatus,
    /// Pinned `thread_call` callback.
    ThreadCall,
    /// Pinned `thread_wakeup` callback.
    ThreadWakeup,
    /// Pinned `thread_getstatus` callback.
    ThreadGetstatus,
    /// Pinned `class_getattributes` callback.
    ClassGetattributes,
    /// Pinned `class_setattributes` callback.
    ClassSetattributes,
    /// Pinned `class_instance` callback.
    ClassInstance,
    /// Pinned `instance_getclass` callback.
    InstanceGetclass,
    /// Pinned `weakref_ref` callback.
    WeakrefRef,
}
impl DefaultFunction {
    /// Original callback symbol; aliases retain this same code identity.
    pub const fn source_symbol(self) -> &'static str {
        match self {
            Self::DefaultDelegateLen => "default_delegate_len",
            Self::TableRawget => "table_rawget",
            Self::TableRawset => "table_rawset",
            Self::TableRawdelete => "table_rawdelete",
            Self::ContainerRawexists => "container_rawexists",
            Self::ObjDelegateWeakref => "obj_delegate_weakref",
            Self::DefaultDelegateTostring => "default_delegate_tostring",
            Self::ObjClear => "obj_clear",
            Self::ArrayAppend => "array_append",
            Self::ArrayExtend => "array_extend",
            Self::ArrayPop => "array_pop",
            Self::ArrayTop => "array_top",
            Self::ArrayInsert => "array_insert",
            Self::ArrayRemove => "array_remove",
            Self::ArrayResize => "array_resize",
            Self::ArrayReverse => "array_reverse",
            Self::ArraySort => "array_sort",
            Self::ArraySlice => "array_slice",
            Self::DefaultDelegateTointeger => "default_delegate_tointeger",
            Self::DefaultDelegateTofloat => "default_delegate_tofloat",
            Self::StringSlice => "string_slice",
            Self::StringFind => "string_find",
            Self::StringTolower => "string_tolower",
            Self::StringToupper => "string_toupper",
            Self::NumberDelegateTochar => "number_delegate_tochar",
            Self::ClosureCall => "closure_call",
            Self::ClosurePcall => "closure_pcall",
            Self::ClosureAcall => "closure_acall",
            Self::ClosurePacall => "closure_pacall",
            Self::ClosureBindenv => "closure_bindenv",
            Self::ClosureGetinfos => "closure_getinfos",
            Self::GeneratorGetstatus => "generator_getstatus",
            Self::ThreadCall => "thread_call",
            Self::ThreadWakeup => "thread_wakeup",
            Self::ThreadGetstatus => "thread_getstatus",
            Self::ClassGetattributes => "class_getattributes",
            Self::ClassSetattributes => "class_setattributes",
            Self::ClassInstance => "class_instance",
            Self::InstanceGetclass => "instance_getclass",
            Self::WeakrefRef => "weakref_ref",
        }
    }
}
/// Native callback identity; default metadata does not imply callable support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFunction {
    /// Bare assertion callback.
    Assert,
    /// Bare print callback.
    Print,
    /// Bare array constructor callback.
    Array,
    /// Bare type callback.
    Type,
    /// A known default delegate callback whose execution remains unsupported.
    Default(DefaultFunction),
}
impl NativeFunction {
    /// Original callback symbol, independent of its registered alias.
    pub const fn source_symbol(self) -> &'static str {
        match self {
            Self::Assert => "base_assert",
            Self::Print => "base_print",
            Self::Array => "base_array",
            Self::Type => "base_type",
            Self::Default(function) => function.source_symbol(),
        }
    }
}
