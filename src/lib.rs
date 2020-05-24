use std::collections::VecDeque;
use std::cmp::Reverse;

type Id = usize;
type NodePtr = Option<Id>;

#[derive(Debug)]
pub enum Error {
    Index(IndexError),
}

#[derive(Debug)]
pub enum IndexError {
    None,
    Empty(Id),
    OutOfBounds(Id),
}

#[derive(Debug)]
struct DirectVecIndex<K,P,V> {
    reuse: Vec<usize>,
    index: Vec<Option<Node<K,P,V>>>,
}
impl<K,P,V> DirectVecIndex<K,P,V> {
    fn new() -> DirectVecIndex<K,P,V> {
        DirectVecIndex {
            reuse: Vec::new(),
            index: Vec::new(),
        }
    }
    fn size(&self) -> usize {
        let i = self.index.len();
        let r = self.reuse.len();
        if i > r { i - r } else { 0 }
    }
    fn insert(&mut self, node: Node<K,P,V>) -> NodePtr {
        Some(match self.reuse.pop() {
            Some(id) => {
                self.index[id] = Some(node);
                id
            },
            None => {
                let id = self.index.len();
                self.index.push(Some(node));
                id
            },
        })
    }
    fn remove(&mut self, id: &NodePtr) -> Result<Node<K,P,V>,IndexError> {
        match id {
            None => Err(IndexError::None),
            Some(id) if *id >= self.index.len() => Err(IndexError::OutOfBounds(*id)),
            Some(id) => match self.index[*id].take() {
                None => Err(IndexError::Empty(*id)),
                Some(node) => {
                    self.reuse.push(*id);
                    Ok(node)
                },
            },
        }
    }
    fn get(&self, id: &NodePtr) -> Result<&Node<K,P,V>,IndexError> {
        match id {
            None => Err(IndexError::None),
            Some(id) => match self.index.get(*id) {
                None => Err(IndexError::OutOfBounds(*id)),
                Some(None) => Err(IndexError::Empty(*id)),
                Some(Some(node)) => Ok(node),
            },
        }
    }
    fn get_mut(&mut self, id: &NodePtr) -> Result<&mut Node<K,P,V>,IndexError> {
        match id {
            None => Err(IndexError::None),
            Some(id) => match self.index.get_mut(*id) {
                None => Err(IndexError::OutOfBounds(*id)),
                Some(None) => Err(IndexError::Empty(*id)),
                Some(Some(node)) => Ok(node),
            },
        }
    }
}

impl<'t,K,P,V> IntoIterator for &'t DirectVecIndex<K,P,V> {
    type Item = (Id, &'t Option<Node<K,P,V>>);
    type IntoIter = std::iter::Enumerate<std::slice::Iter<'t,Option<Node<K,P,V>>>>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.index.iter().enumerate()
    }
}

type Index<K,P,V> = DirectVecIndex<K,P,V>;

#[derive(Debug,Clone,Copy)]
struct Node<K,P,V> {
    key: K,
    priority: P,
    value: V,
    left: NodePtr,
    right: NodePtr,
}

#[derive(Debug)]
struct Split<K,P,V> {
    left: NodePtr,
    entry: NodePtr,
    right: NodePtr,
    index: Index<K,P,V>,
}

#[derive(Debug)]
pub struct Treap<K,P,V> {
    root: NodePtr,
    index: Index<K,P,V>,
}
impl<K: PartialOrd + PartialEq,P: PartialOrd,V> Treap<K,P,V> {
    pub fn new() -> Treap<K,P,V> {
        Treap{ root: None, index: Index::new() }
    }
    pub fn len(&self) -> usize {
        self.index.size()
    }
    pub fn insert(&mut self, key: K, priority: P, value: V) -> Result<Option<(P,V)>,Error> {
        let mut tmp = Treap { root: None, index: Index::new() };
        std::mem::swap(&mut tmp, self);
        let spl = tmp.split(&key).map_err(Error::Index)?;
        let new_node = Node { key: key, priority: priority, value: value, left: None, right: None };
        let mut index = spl.index;
        let left = spl.left;
        let right = spl.right;
        let node = index.remove(&spl.entry).ok();

        let new = index.insert(new_node);
        let root = Treap::merge_nodes(&mut index,left,new).map_err(Error::Index)?;
        *self = Treap {
            root: Treap::merge_nodes(&mut index,root,right).map_err(Error::Index)?,
            index: index,
        };
        
        Ok(node.map(|node| (node.priority,node.value)))
    }
    pub fn remove(&mut self, key: &K) -> Result<Option<(P,V)>,Error> {
        let mut tmp = Treap { root: None, index: Index::new() };
        std::mem::swap(&mut tmp, self);
        let spl = tmp.split(&key).map_err(Error::Index)?;

        let mut index = spl.index;
        let left = spl.left;
        let right = spl.right;
        let node = index.remove(&spl.entry).ok();

        *self = Treap {
            root: Treap::merge_nodes(&mut index,left,right).map_err(Error::Index)?,
            index: index,
        };
        
        Ok(node.map(|node| (node.priority,node.value)))
    }
    pub fn get<'t>(&'t self, key: &K) -> Result<Option<(&'t P, &'t V)>,Error> {
        fn search_node<'t,K: PartialOrd + PartialEq,P,V>(index: &'t Index<K,P,V>, node: NodePtr, key: &K) -> Result<Option<(&'t P, &'t V)>,IndexError> {
            if node.is_none() { return Ok(None); }
            let entry = index.get(&node)?;
            if entry.key == *key {
                Ok(Some((&entry.priority,&entry.value)))
            } else {
                if entry.key > *key {
                    search_node(index,entry.left,key)
                } else {
                    search_node(index,entry.right,key)
                }
            }
        }

        search_node(&self.index,self.root,key).map_err(Error::Index)
    }
    pub fn get_mut<'t>(&'t mut self, key: &K) -> Result<Option<(&'t P, &'t mut V)>,Error> {
        enum Action {
            Found(NodePtr),
            Left(NodePtr),
            Right(NodePtr),
        }
        fn search_node<'t,K: PartialOrd + PartialEq,P,V>(index: &'t mut Index<K,P,V>, node: NodePtr, key: &K) -> Result<Option<(&'t P, &'t mut V)>,IndexError> {
            if node.is_none() { return Ok(None); }
            let action = {
                let entry = index.get_mut(&node)?;
                if entry.key == *key {
                    Action::Found(node)
                } else {
                    if entry.key > *key {
                        Action::Left(entry.left)
                    } else {
                        Action::Right(entry.right)
                    }
                }               
            };
            match action {
                Action::Found(node) => {
                    let node_ref = index.get_mut(&node)?;
                    Ok(Some((&node_ref.priority,&mut node_ref.value)))
                },
                Action::Left(left) => search_node(index,left,key),
                Action::Right(right) => search_node(index,right,key),
            }
        }

        search_node(&mut self.index,self.root,key).map_err(Error::Index)
    }
    pub fn priority<'t>(&'t self, key: &K) -> Result<Option<&'t P>,Error> {
        fn search_node<'t,K: PartialOrd + PartialEq,P,V>(index: &'t Index<K,P,V>, node: NodePtr, key: &K) -> Result<Option<&'t P>,IndexError> {
            if node.is_none() { return Ok(None); }
            let entry = index.get(&node)?;
            if entry.key == *key {
                Ok(Some(&entry.priority))
            } else {
                if entry.key > *key {
                    search_node(index,entry.left,key)
                } else {
                    search_node(index,entry.right,key)
                }
            }
        }

        search_node(&self.index,self.root,key).map_err(Error::Index)
    }
    pub fn prioritize(&mut self, key: &K, new_p: P) -> Result<Option<P>,Error> {
        let mut tmp = Treap { root: None, index: Index::new() };
        std::mem::swap(&mut tmp, self);
        let spl = tmp.split(&key).map_err(Error::Index)?;
        
        let mut index = spl.index;
        let left = spl.left;
        let right = spl.right;
        let (old_p,new) = match index.remove(&spl.entry).ok() {
            Some(node) => {
                let new_node = Node { key: node.key, priority: new_p, value: node.value, left: None, right: None };
                (Some(node.priority),index.insert(new_node))
            },
            None => (None,None),
        };

        let root = Treap::merge_nodes(&mut index,left,new).map_err(Error::Index)?;
        *self = Treap {
            root: Treap::merge_nodes(&mut index,root,right).map_err(Error::Index)?,
            index: index,
        };
        
        Ok(old_p)
    }
    pub fn pop(&mut self) -> Result<Option<(K,P,V)>,Error> {
        if self.root.is_none() { return Ok(None); }
        let node = self.index.remove(&self.root.take()).map_err(Error::Index)?;
        self.root = Treap::merge_nodes(&mut self.index,node.left,node.right).map_err(Error::Index)?;
        Ok(Some((node.key,node.priority,node.value)))
    }
    pub fn depth(&self) -> Result<usize,Error> {
        fn depth_node<K,P,V>(index: &Index<K,P,V>, node: NodePtr) -> Result<usize,IndexError> {
            if node.is_none() { return Ok(0); }
            let (l,r) = {
                let entry = index.get(&node)?;
                (entry.left,entry.right)
            };

            Ok(1 + usize::max(depth_node(index,l)?,depth_node(index,r)?))
        }

        depth_node(&self.index, self.root).map_err(Error::Index)
    }
    pub fn cut(&mut self, p: &P) -> Result<(),Error> {
        fn check_node<'t,K,P: PartialOrd,V>(index: &'t mut Index<K,P,V>, node: NodePtr, p: &P) -> Result<bool,IndexError> {
            if node.is_none() { return Ok(true); }
            let entry = index.get(&node)?;
            match entry.priority < *p {
                true => {
                    drop_node(index,node,p)?;
                    Ok(true)
                },
                false => {
                    let (l,r) = (entry.left,entry.right);
                    if check_node(index,l,p)? { index.get_mut(&node)?.left = None; }
                    if check_node(index,r,p)? { index.get_mut(&node)?.right = None; }
                    Ok(false)
                }
            }
        }
        fn drop_node<'t,K,P,V>(index: &'t mut Index<K,P,V>, node: NodePtr, p: &P) -> Result<(),IndexError> {
            if node.is_none() { return Ok(()); }
            let entry = index.remove(&node)?;
            drop_node(index,entry.left,p)?;
            drop_node(index,entry.right,p)
        }

        if check_node(&mut self.index,self.root,p).map_err(Error::Index)? {
            self.root = None;
        }
        Ok(())
    }
}
impl<K,P: Ord,V> Treap<K,P,V> {
    pub fn nth_priority(&self, n: usize) -> Result<Option<&P>,Error> {
        fn nth_priority_node<'t,K,P: Ord,V>(index: &'t Index<K,P,V>, node: NodePtr, n: usize, queue: &mut VecDeque<NodePtr>, pri: &mut Vec<Reverse<&'t P>>) -> Result<(),IndexError> {
            if node.is_none() { return Ok(()); }
            let entry = index.get(&node)?;
            let (push,check_ch) = match pri.binary_search(&Reverse(&entry.priority)) {
                Ok(i) if i < n => (None,true),
                Err(i) if i < n => (Some(i),true),
                _ => (None,false),
            };
            if let Some(i) = push {
                pri.insert(i,Reverse(&entry.priority));
            }
            if check_ch {
                if entry.left.is_some() { queue.push_back(entry.left); }
                if entry.right.is_some() { queue.push_back(entry.right); }
            }
            Ok(())
        }
        
        let mut queue = VecDeque::new();
        let mut pri = Vec::new();

        nth_priority_node(&self.index,self.root,n,&mut queue,&mut pri).map_err(Error::Index)?;       
        while let Some(node) = queue.pop_front() {
            nth_priority_node(&self.index,node,n,&mut queue,&mut pri).map_err(Error::Index)?;           
        }
        if pri.len() >= n { Ok(Some(pri[n-1].0)) } else { Ok(None) }
    }
}

impl<K: PartialOrd, P: PartialOrd, V> Treap<K,P,V> {    
    fn split(self, key: &K) -> Result<Split<K,P,V>,IndexError> {
        fn split_nodes<K: PartialOrd,P,V>(index: &mut Index<K,P,V>, node: NodePtr, key: &K) -> Result<(NodePtr,NodePtr,NodePtr),IndexError> { // left, entry, right
            if node.is_none() { return Ok((None,None,None)); }
            let entry = index.get(&node)?;
            if entry.key == *key {
                let (l,r) = (entry.left,entry.right);
                let mut v = index.get_mut(&node)?;
                v.left = None;
                v.right = None;
                Ok((l,node,r))
            } else {
                if entry.key > *key {
                    // left
                    let nxt = entry.left;
                    let (l,e,r) = split_nodes(index, nxt, key)?;
                    index.get_mut(&node)?.left = r;
                    Ok((l,e,node))
                } else {
                    // right
                    let nxt =  entry.right;
                    let (l,e,r) = split_nodes(index, nxt, key)?;
                    index.get_mut(&node)?.right = l;
                    Ok((node,e,r))
                }
            }
        }
        
        let mut index = self.index;
        let (l,e,r) = split_nodes(&mut index,self.root,key)?;
        Ok(Split{ left: l, entry: e, right: r, index: index })
    }
    fn merge_nodes(index: &mut Index<K,P,V>, left: NodePtr, right: NodePtr) -> Result<NodePtr,IndexError> {
        if left.is_none() { return Ok(right); }
        if right.is_none() { return Ok(left); }
        let (left_p,left_right) = {
            let entry = index.get(&left)?;
            (&entry.priority,entry.right)
        };
        let (right_p,right_left) = {
            let entry = index.get(&right)?;
            (&entry.priority,entry.left)
        };
        if left_p > right_p {
            index.get_mut(&left)?.right = Treap::merge_nodes(index, left_right, right)?;
            Ok(left)
        } else {
            index.get_mut(&right)?.left = Treap::merge_nodes(index, left,right_left)?;
            Ok(right)
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn it_works() {
        let mut treap: Treap<u64,u64,()> = Treap::new();
        treap.insert(7,10,()).unwrap();
        treap.insert(4,6,()).unwrap();
        treap.insert(13,8,()).unwrap();
        treap.insert(2,4,()).unwrap();
        treap.insert(6,2,()).unwrap();
        treap.insert(9,7,()).unwrap();
        treap.insert(14,4,()).unwrap();
        treap.insert(0,3,()).unwrap();
        treap.insert(3,3,()).unwrap();
        treap.insert(5,1,()).unwrap();
        treap.insert(11,3,()).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("");
        let k = 5;
        let spl = treap.split(&k).unwrap();
        println!("{}: {:?} {:?} {:?}",k,spl.left,spl.entry,spl.right);
        for k in &spl.index {
            println!("{:?}",k);
        }
        panic!("");
    }

    #[test]
    fn it_works_2() {
        let mut treap: Treap<u64,u64,()> = Treap::new();
        treap.insert(7,10,()).unwrap();
        treap.insert(4,6,()).unwrap();
        treap.insert(13,8,()).unwrap();
        treap.insert(2,4,()).unwrap();
        treap.insert(6,2,()).unwrap();
        treap.insert(9,7,()).unwrap();
        treap.insert(14,4,()).unwrap();
        treap.insert(0,3,()).unwrap();
        treap.insert(3,3,()).unwrap();
        treap.insert(5,1,()).unwrap();
        treap.insert(11,3,()).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("");
        let k = 12;
        let spl = treap.split(&k).unwrap();
        println!("{}: {:?} {:?} {:?}",k,spl.left,spl.entry,spl.right);
        for k in &spl.index {
            println!("{:?}",k);
        }
        panic!("");
    }

    #[test]
    fn insert() {
        let mut treap: Treap<u64,u64,()> = Treap::new();
        treap.insert(7,10,()).unwrap();
        treap.insert(4,6,()).unwrap();
        treap.insert(13,8,()).unwrap();
        treap.insert(2,4,()).unwrap();
        treap.insert(6,2,()).unwrap();
        treap.insert(9,7,()).unwrap();
        treap.insert(14,4,()).unwrap();
        treap.insert(0,3,()).unwrap();
        treap.insert(3,3,()).unwrap();
        treap.insert(5,1,()).unwrap();
        treap.insert(11,3,()).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("\nInsert (5,8) -> {:?}\n",treap.insert(5,8,()).unwrap());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        panic!("");
    }

    #[test]
    fn pop() {
        let mut treap: Treap<u64,u64,()> = Treap::new();
        treap.insert(7,10,()).unwrap();
        treap.insert(4,6,()).unwrap();
        treap.insert(13,8,()).unwrap();
        treap.insert(2,4,()).unwrap();
        treap.insert(6,2,()).unwrap();
        treap.insert(9,7,()).unwrap();
        treap.insert(14,4,()).unwrap();
        treap.insert(0,3,()).unwrap();
        treap.insert(3,3,()).unwrap();
        treap.insert(5,1,()).unwrap();
        treap.insert(11,3,()).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        /*println!("\nInsert (5,8) -> {:?}\n",treap.insert(5,8,()).unwrap());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }*/
        println!("\nPop: {:?}",treap.pop());
        println!("Pop: {:?}",treap.pop());
        println!("Pop: {:?}",treap.pop());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        panic!("");
    }

    #[test]
    fn get() {
        let mut treap: Treap<u64,u64,(u64,u64)> = Treap::new();
        treap.insert(7,10,(7,10)).unwrap();
        treap.insert(4,6,(4,6)).unwrap();
        treap.insert(13,8,(13,8)).unwrap();
        treap.insert(2,4,(2,4)).unwrap();
        treap.insert(6,2,(6,2)).unwrap();
        treap.insert(9,7,(9,7)).unwrap();
        treap.insert(14,4,(14,4)).unwrap();
        treap.insert(0,3,(0,3)).unwrap();
        treap.insert(3,3,(3,3)).unwrap();
        treap.insert(5,1,(5,1)).unwrap();
        treap.insert(11,3,(11,3)).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("\nInsert (5,8) -> {:?}\n",treap.insert(5,8,(5,8)).unwrap());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("\nGet 5: {:?}",treap.get(&5));
        println!("Get 7: {:?}",treap.get(&7));
        println!("Get 8: {:?}",treap.get(&8));
        println!("Get 13: {:?}",treap.get(&13));
        panic!("");
    }

    #[test]
    fn priority() {
        let mut treap: Treap<u64,u64,(u64,u64)> = Treap::new();
        treap.insert(7,10,(7,10)).unwrap();
        treap.insert(4,6,(4,6)).unwrap();
        treap.insert(13,8,(13,8)).unwrap();
        treap.insert(2,4,(2,4)).unwrap();
        treap.insert(6,2,(6,2)).unwrap();
        treap.insert(9,7,(9,7)).unwrap();
        treap.insert(14,4,(14,4)).unwrap();
        treap.insert(0,3,(0,3)).unwrap();
        treap.insert(3,3,(3,3)).unwrap();
        treap.insert(5,1,(5,1)).unwrap();
        treap.insert(11,3,(11,3)).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        /*println!("\nInsert (5,8) -> {:?}\n",treap.insert(5,8,(5,8)).unwrap());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }*/
        println!("\nPri 5: {:?}",treap.priority(&5));
        println!("Pri 7: {:?}",treap.priority(&7));
        println!("Pri 8: {:?}",treap.priority(&8));
        println!("Pri 13: {:?}",treap.priority(&13));
        panic!("");
    }

    #[test]
    fn remove() {
        let mut treap: Treap<u64,u64,(u64,u64)> = Treap::new();
        treap.insert(7,10,(7,10)).unwrap();
        treap.insert(4,6,(4,6)).unwrap();
        treap.insert(13,8,(13,8)).unwrap();
        treap.insert(2,4,(2,4)).unwrap();
        treap.insert(6,2,(6,2)).unwrap();
        treap.insert(9,7,(9,7)).unwrap();
        treap.insert(14,4,(14,4)).unwrap();
        treap.insert(0,3,(0,3)).unwrap();
        treap.insert(3,3,(3,3)).unwrap();
        treap.insert(5,1,(5,1)).unwrap();
        treap.insert(11,3,(11,3)).unwrap();
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("\nInsert (5,8) -> {:?}\n",treap.insert(5,8,(5,8)).unwrap());
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        println!("\nRemove 5: {:?}",treap.remove(&5));
        println!("Remove 7: {:?}",treap.remove(&7));
        println!("Remove 8: {:?}",treap.remove(&8));
        println!("Remove 13: {:?}",treap.remove(&13));
        println!("{:?}",treap.root);
        for k in &treap.index {
            println!("{:?}",k);
        }
        panic!("");
    }
}*/

