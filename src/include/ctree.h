/*
AshOS n-array tree library
Copyright (C) 2022  AshOS

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.

	original author: Jan Novotný (https://github.com/CuBeRJAN)
*/
#ifndef __CTREE_H_
#define __CTREE_H_

#include "vectors.h"


typedef struct v_tree {
  v_str* _names;
  v_int* _relations;
} v_tree;

v_tree* v_tree_new(); // Allocate new tree, 'v_tree* tree = v_tree_new()', has to be free'd after
void v_tree_free(v_tree*); // free tree

int v_tree_add_node(v_tree*, int parent); // add new node, return it's id
int v_tree_get_parent(v_tree*, int node);
void v_tree_set_parent(v_tree*, int node, int parent);

v_int* v_tree_get_children(v_tree*, int parent); // use like so: v_int* children = v_tree_get_children(tree, parent); free with v_int_free(children) after
v_int* v_tree_path_to_root(v_tree*, int node);
v_int* v_tree_path_from_root(v_tree*, int node);
v_int* v_tree_get_all_children(v_tree*, int parent);

void v_tree_set_name(v_tree*, int node, char* name);
string v_tree_get_name(v_tree*, int node);
void v_tree_remove_node(v_tree*, int node);
void v_tree_print(v_tree*);

// for internal library use, not to be used by users
int v_tree__get_depth(v_tree*, int node);
v_int* v_tree__sorted_crawl(v_tree*);
void v_tree__r_last_node(v_tree*, int node);

#endif
