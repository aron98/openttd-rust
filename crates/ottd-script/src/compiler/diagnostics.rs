//! Compiler exceptions are distinct from public typed failures and VM diagnostics.
use crate::{ByteString, CompileError, CompileErrorKind, Realm};
use std::borrow::Cow;

#[expect(
    clippy::redundant_pub_crate,
    reason = "Only crate compilation paths may carry detailed failures"
)]
#[derive(Debug)]
pub(crate) struct CompileFailure {
    pub(crate) error: CompileError,
    message: Option<Cow<'static, [u8]>>,
}
impl CompileFailure {
    pub(crate) const fn new(offset: usize, kind: CompileErrorKind) -> Self {
        Self {
            error: CompileError { offset, kind },
            message: None,
        }
    }
    pub(crate) fn native(mut self, message: &'static str) -> Self {
        self.message = Some(Cow::Borrowed(message.as_bytes()));
        self
    }
    pub(crate) fn expected(mut self, symbol: u8) -> Self {
        self.message = Some(Cow::Owned(
            format!("expected '{}'", char::from(symbol)).into_bytes(),
        ));
        self
    }
    pub(crate) fn invalid_constant(mut self, name: &[u8], member: &[u8]) -> Self {
        let mut message = b"invalid constant [".to_vec();
        message.extend_from_slice(name);
        message.push(b'.');
        message.extend_from_slice(member);
        message.push(b']');
        self.message = Some(Cow::Owned(message));
        self
    }
    pub(crate) fn native_message(&self, realm: &Realm) -> Option<ByteString> {
        self.message.as_deref().map(|message| {
            // CompileException::what() is adopted as a C string by SQCompiler::Compile.
            let end = message
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(message.len());
            realm.string(message.get(..end).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CompileFailure;
    use crate::{CompileErrorKind, Realm};

    #[test]
    fn compiler_exception_adoption_stops_at_nul() {
        let realm = Realm::new();
        let failure = CompileFailure::new(7, CompileErrorKind::ExpectedToken)
            .invalid_constant(b"first\0unreachable", b"member");
        assert_eq!(
            failure.native_message(&realm).expect("message").as_bytes(),
            b"invalid constant [first"
        );
        // Synthetic adoption test only: source identifier tokens cannot contain NUL.
        assert_eq!(failure.error.offset, 7);
    }
}
