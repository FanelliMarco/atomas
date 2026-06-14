use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Node<T: Clone> {
    pub value: T,
    pub next: Option<Rc<RefCell<Node<T>>>>,
    pub prev: Option<Rc<RefCell<Node<T>>>>,
}

#[derive(Clone, Debug)]
pub struct CircularList<T: Clone> {
    pub head: Option<Rc<RefCell<Node<T>>>>,
    pub size: usize,
}

impl<T: Clone + Debug> CircularList<T> {
    pub fn new() -> Self {
        CircularList {
            head: None,
            size: 0,
        }
    }

    pub fn insert(&mut self, value: T, index: usize) {
        let new_node = Rc::new(RefCell::new(Node {
            value,
            next: None,
            prev: None,
        }));

        if self.head.is_none() {
            new_node.borrow_mut().next = Some(Rc::clone(&new_node));
            new_node.borrow_mut().prev = Some(Rc::clone(&new_node));
            self.head = Some(new_node);
        } else {
            let mut current = self.head.as_ref().unwrap().borrow().next.clone();
            for _ in 0..index {
                let next = current.as_ref().unwrap().borrow().next.clone();
                current = next;
            }

            let next = current.as_ref().unwrap().borrow().next.clone();

            new_node.borrow_mut().next = next.clone();
            new_node.borrow_mut().prev = current.clone();

            current.as_ref().unwrap().borrow_mut().next = Some(Rc::clone(&new_node));
            next.as_ref().unwrap().borrow_mut().prev = Some(Rc::clone(&new_node));

            if index == 0 {
                self.head = Some(Rc::clone(&new_node));
            }
        }

        self.size += 1;
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn push(&mut self, value: T) {
        self.insert(value, self.size);
    }
    
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

pub struct CircularListIterator<T: Clone> {
    current: Option<Rc<RefCell<Node<T>>>>,
    end: Option<Rc<RefCell<Node<T>>>>,
    first: bool,
}

impl<T: Clone + Debug> Default for CircularList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Debug> Iterator for CircularListIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.as_ref()?;

        if !self.first && Rc::ptr_eq(current, self.end.as_ref().unwrap()) {
            return None;
        }

        let value = current.borrow().value.clone();
        let next = current.borrow().next.clone();

        self.current = next;
        self.first = false;

        Some(value)
    }
}

impl<T: Clone + Debug> CircularList<T> {
    pub fn iter(&self) -> CircularListIterator<T> {
        CircularListIterator {
            current: self.head.clone(),
            end: self.head.clone(),
            first: true,
        }
    }
}
