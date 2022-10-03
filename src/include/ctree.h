#ifndef __CTREE_H_
#define __CTREE_H_

#include<stdlib.h>
#include<string.h>
#include<stdio.h>
#include "vectors.h"


typedef struct v_tree {
//  v_str* _names;
  v_int* _relations;
} v_tree;

v_tree* v_tree_new () {
  v_tree* tree = malloc(sizeof(v_tree));
  tree->_relations = v_int_new(0);
  v_int_push(tree->_relations, -1);
  return tree;
}

void v_tree_free(v_tree* tree) {
  v_int_free(tree->_relations);
  free(tree);
}

int v_tree_add_node(v_tree* tree, int parent) {
  v_int_push(tree->_relations, parent);
  return tree->_relations->len;
}

int v_tree_get_parent(v_tree* tree, int node) {
  return tree->_relations->data[node];
}

void v_tree_set_parent(v_tree* tree, int node, int parent) {
  tree->_relations->data[node] = parent;
}

v_int* v_tree_get_children(v_tree* tree, int parent) {
  v_int* found = v_int_new(0);
  for (size_t i = 0; i < tree->_relations->len; i++) {
    if (tree->_relations->data[i] == parent)
      v_int_push(found, i);
  }
  return found;
}

#endif
