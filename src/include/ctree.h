#ifndef __CTREE_H_
#define __CTREE_H_

#include<stdlib.h>
#include<string.h>
#include<stdio.h>
#include<stdbool.h>
#include "vectors.h"


typedef struct v_tree {
  v_str* _names;
  v_int* _relations;
  v_int* _nums;
} v_tree;

v_tree* v_tree_new () {
  v_tree* tree = malloc(sizeof(v_tree));
  tree->_relations = v_int_new(0);
  v_int_push(tree->_relations, -1);
  tree->_names = v_str_new(0);
  string n;
  str_set(&n, "");
  v_str_push(tree->_names, n);
  return tree;
}

void v_tree_free(v_tree* tree) {
  v_int_free(tree->_relations);
  v_str_free(tree->_names);
  free(tree);
}

int v_tree_add_node(v_tree* tree, int parent) {
  v_int_push(tree->_relations, parent);
  string n;
  str_set(&n, "");
  v_str_push(tree->_names, n);
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
  v_str_erase(tree->_names, node, 1);
  for (size_t i = 0; i < tree->_relations->len; i++) {
    if (tree->_relations->data[i] > node) tree->_relations->data[i]--;
  }
}

void v_tree_set_name(v_tree* tree, int node, char* name) {
  free(tree->_names->data[node].str);
  str_set(&tree->_names->data[node], name);
}

string v_tree_get_name(v_tree* tree, int node) {
  return tree->_names->data[node];
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

int v_tree__get_depth(v_tree* tree, int node) {
  int parent = v_tree_get_parent(tree, node);
  int depth = 0;
  while (parent != -1) {
    parent = v_tree_get_parent(tree, parent);
    depth++;
  }
  return depth;
}

v_int* v_tree__sorted_crawl(v_tree* tree) {
  int counter = 0;
  v_int* sorted = v_int_new(0);
  v_int* indexes = v_int_new(0);
  v_int_push(indexes, 0);
  while (true) {
    v_int* found = v_tree_get_children(tree, counter);
    if (found->len > indexes->data[v_tree__get_depth(tree, counter)]) {
      counter = found->data[indexes->data[v_tree__get_depth(tree, counter)]];
      v_int_push(sorted, counter);
      int counter_depth = v_tree__get_depth(tree, counter);
      if (indexes->len > counter_depth + 1) {
        indexes->data[counter_depth + 1] = 0;
        for (int i = counter_depth; i < indexes->len; i++) {
          indexes->data[i] = 0;
        }
      }
      else {
        v_int_push(indexes, 0);
      }
    }
    else {
      counter = tree->_relations->data[counter];
      if (counter == -1) {
        v_int_free(found);
        break;
      }
      indexes->data[v_tree__get_depth(tree, counter)] += 1;
    }
    v_int_free(found);
  }
  v_int_free(indexes);
  return sorted;
}

void v_tree_print(v_tree* tree) {
  int maxdepth = 0;
  for (int i = 0; i < tree->_relations->len; i++) {
    int d = v_tree__get_depth(tree, i);
    if (d > maxdepth)
      maxdepth = d;
  }
  v_int* sorted = v_tree__sorted_crawl(tree);
  int exs = 0;
  int nypos = 0;

  char** matrix = malloc(sizeof(char*) * (sorted->len + 1));
  for (int i = 0; i < sorted->len + 1; ++i) {
    matrix[i] = malloc(sizeof(char) * (maxdepth + 1));
  }

  printf("0 - %s \n", tree->_names->data[0].str);
  for (int loop = 0; loop < 5; loop++) {
    for (int i = 0; i < sorted->len; i++) {
      for (int j = 0; j < maxdepth; j++) {
        if (loop == 0) {
          if (j < v_tree__get_depth(tree, sorted->data[i]))
            matrix[i][j] = '.';
          else
            matrix[i][j] = ' ';
        }
        else if (loop == 1) {
          if (matrix[i][j] == '.' && matrix[i][j+1] == ' ') matrix[i][j] = 'x';
          if (matrix[i][j] == '.' && j == maxdepth - 1) matrix[i][j] = 'x';
        }
        else if (loop == 2) {
          if (matrix[i][j] == 'x') {
            for (int k = i + 1; k < sorted->len; k++) {
              if (matrix[k][j] != '.' && matrix[k][j] != 'x') break;
              if (matrix[k][j] == 'x') exs++;
            }
          }
          nypos = i + 1;
          while (exs != 0) {
            if (matrix[nypos][j] == '.') matrix[nypos][j] = 'x';
            else if (matrix[nypos][j] == 'x') --exs;
            nypos++;
          }
        }
        else if (loop == 3) {
          if (matrix[i][j] == 'x') {
            if (matrix[i][j + 1] != 'x' && matrix[i + 1][j] == 'x') matrix[i][j] = '+';
            if (matrix[i][j + 1] != 'x' && matrix[i + 1][j] != 'x') matrix[i][j] = 'L';
            if (j != maxdepth - 1)
              if ((matrix[i][j + 1] == 'x' || matrix[i][j + 1] == '.') && matrix[i + 1][j] != ' ') matrix[i][j] = '|';

          }
        }
        else if (loop == 4) {
          if (matrix[i][j] == '+') printf("├── ");
          else if (matrix[i][j] == 'L') printf("└── ");
          else if (matrix[i][j] == '|') printf("│   ");
          else if (matrix[i][j] == '.') printf("    ");
        }
      }
      if (loop == 4) {
        printf("%d - %s \n", sorted->data[i], tree->_names->data[sorted->data[i]].str);
      }
    }
  }
  for (int i = 0; i < sorted->len + 1; ++i) {
    free(matrix[i]);
  }
  free(matrix);
  v_int_free(sorted);
}


#endif
