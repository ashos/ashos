#ifndef __CTREE_H_
#define __CTREE_H_

#include<stdlib.h>
#include<string.h>
#include<stdio.h>
#include<stdbool.h>
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

v_int* v_tree_path_to_root(v_tree* tree, int node) {
  v_int* path = v_int_new(0);
  while (tree->_relations->data[node] != -1) {
    v_int_push(path, node);
    node = tree->_relations->data[node];
  }
  v_int_push(path, node);
  return path;
}

v_int* v_tree_path_from_root(v_tree* tree, int node) {
  v_int* path = v_tree_path_to_root(tree, node);
  v_int_reverse(path);
  return path;
}

bool v_tree_in_range(v_tree* tree, size_t val) {
  return val < tree->_relations->len;
}

v_int* v_tree_get_all_children(v_tree* tree, int parent) {
  v_int* found = v_tree_get_children(tree, parent);
  int i = 0;
  while (i < found->len) {
    v_int* nfound = v_tree_get_children(tree, found->data[i]);
    v_int_cat(found, nfound);
    v_int_free(nfound);
    ++i;
  }
  return found;
}

void v_tree__r_last_node(v_tree* tree, int node) {
  v_int_erase(tree->_relations, node, 1);
  for (size_t i = 0; i < tree->_relations->len; i++) {
    if (tree->_relations->data[i] > node) tree->_relations->data[i]--;
  }
}

void v_tree_remove_node(v_tree* tree, int node) {
  v_int* found = v_tree_get_all_children(tree, node);
  while (found->len != 0) {
    v_tree__r_last_node(tree, found->data[found->len - 1]);
    v_int_erase(found, found->len - 1, 1);
    for (int i = 0; i < found->len; i++) {
      if (found->data[i] > found->data[found->len - 1])
        found->data[i]--;
    }
  }
  v_int_free(found);
}

#endif
