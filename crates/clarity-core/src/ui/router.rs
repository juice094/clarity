//! Generic navigation stack.
//!
//! A minimal, notedeck-inspired router: a `Vec<R>` back-stack with push/pop/peek.
//! Layer semantics (main/modal/right-rail) are owned by the caller; this type
//! only knows how to manage one stack of cloneable routes.

/// A simple navigation back-stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Router<R: Clone + PartialEq> {
    /// Stack of routes. The last element is the currently active route.
    stack: Vec<R>,
}

impl<R: Clone + PartialEq> Router<R> {
    /// Create an empty router. Equivalent to `Router::default()` but does not
    /// require `R: Default`.
    pub fn empty() -> Self {
        Self { stack: Vec::new() }
    }

    /// Create a router seeded with an initial route.
    pub fn new(initial: R) -> Self {
        Self {
            stack: vec![initial],
        }
    }

    /// Push a new route onto the stack, making it active.
    ///
    /// If `route` is identical to the currently active route, the stack is left
    /// unchanged so repeated taps of the same navigation item do not create
    /// useless history entries.
    pub fn navigate(&mut self, route: R) {
        if self.stack.last() != Some(&route) {
            self.stack.push(route);
        }
    }

    /// Replace the current route without adding history.
    pub fn replace(&mut self, route: R) {
        if let Some(last) = self.stack.last_mut() {
            *last = route;
        } else {
            self.stack.push(route);
        }
    }

    /// Pop the current route and return it, if there is more than one route
    /// in the stack. Returns `None` when popping would leave the stack empty,
    /// so the caller can decide whether to close the layer or ignore.
    pub fn go_back(&mut self) -> Option<R> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Pop the current route regardless of stack depth.
    ///
    /// Returns `None` if the stack is already empty.
    pub fn pop(&mut self) -> Option<R> {
        self.stack.pop()
    }

    /// Reference to the currently active route, if any.
    pub fn current(&self) -> Option<&R> {
        self.stack.last()
    }

    /// True when the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// True when `go_back` would return a route (i.e. there is history).
    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// Number of routes in the stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Clear all history and set the current route.
    pub fn reset(&mut self, route: R) {
        self.stack.clear();
        self.stack.push(route);
    }

    /// Clear the entire stack.
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestRoute {
        A,
        B,
        C,
    }

    #[test]
    fn router_starts_with_initial_route() {
        let router = Router::new(TestRoute::A);
        assert_eq!(router.current(), Some(&TestRoute::A));
        assert!(!router.can_go_back());
    }

    #[test]
    fn navigate_pushes_and_current_updates() {
        let mut router = Router::new(TestRoute::A);
        router.navigate(TestRoute::B);
        assert_eq!(router.current(), Some(&TestRoute::B));
        assert!(router.can_go_back());
    }

    #[test]
    fn navigate_deduplicates_current_route() {
        let mut router = Router::new(TestRoute::A);
        router.navigate(TestRoute::B);
        router.navigate(TestRoute::B);
        assert_eq!(router.len(), 2);
        assert_eq!(router.current(), Some(&TestRoute::B));
        assert_eq!(router.go_back(), Some(TestRoute::B));
        assert_eq!(router.current(), Some(&TestRoute::A));
    }

    #[test]
    fn go_back_returns_to_previous_route() {
        let mut router = Router::new(TestRoute::A);
        router.navigate(TestRoute::B);
        let back = router.go_back();
        assert_eq!(back, Some(TestRoute::B));
        assert_eq!(router.current(), Some(&TestRoute::A));
    }

    #[test]
    fn go_back_never_leaves_stack_empty() {
        let mut router = Router::new(TestRoute::A);
        assert_eq!(router.go_back(), None);
        assert_eq!(router.current(), Some(&TestRoute::A));
    }

    #[test]
    fn replace_updates_current_without_history() {
        let mut router = Router::new(TestRoute::A);
        router.navigate(TestRoute::B);
        router.replace(TestRoute::C);
        assert_eq!(router.current(), Some(&TestRoute::C));
        assert!(router.can_go_back());
        assert_eq!(router.go_back(), Some(TestRoute::C));
        assert_eq!(router.current(), Some(&TestRoute::A));
    }

    #[test]
    fn reset_clears_history() {
        let mut router = Router::new(TestRoute::A);
        router.navigate(TestRoute::B);
        router.reset(TestRoute::C);
        assert_eq!(router.current(), Some(&TestRoute::C));
        assert!(!router.can_go_back());
    }
}
