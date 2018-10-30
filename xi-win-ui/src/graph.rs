// Copyright 2018 The xi-editor Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Graph for structure for widget tree.

use Id;

#[derive(Default)]
pub struct Graph {
    pub root: Id,
    pub children: Vec<Vec<Id>>,
    pub parent: Vec<Id>,
}

impl Graph {
    pub fn alloc_node(&mut self) -> Id {
        let id = self.children.len();
        self.children.push(vec![]);
        self.parent.push(id);
        id
    }

    pub fn append_child(&mut self, parent: Id, child: Id) {
        self.children[parent].push(child);
        self.parent[child] = parent;
    }

    /// Remove the child from the parent.
    ///
    /// Can panic if the graph structure is invalid. This function leaves the
    /// child in an unparented state, i.e. it can be added again.
    pub fn remove_child(&mut self, parent: Id, child: Id) {
        let ix = self.children[parent].iter().position(|&x| x == child)
            .expect("tried to remove nonexistent child");
        self.children[parent].remove(ix);
        self.parent[child] = child;
    }
}
