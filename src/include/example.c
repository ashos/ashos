#include<stdlib.h>
#include<string.h>
#include<stdio.h>
#include "vectors.h"
#include "ctree.h"


int main(void) {
  v_tree* tree = v_tree_new(); // Create new tree

  v_tree_add_node(tree, 0); // Add to node 0
  v_tree_add_node(tree, 0);
  v_tree_add_node(tree, 1);

  v_int* children = v_tree_get_children(tree, 0); // get list of children (not recursive currently)
  printf("%d", children->len); // print number of children (2 in this case)
  v_int_free(children); // deallocate memory

  v_tree_free(tree); // Deallocate memory

  return 0;
}
