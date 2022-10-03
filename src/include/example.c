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
  v_tree_set_name(tree, 1, "test of");
  v_tree_set_name(tree, 3, "names");
  v_tree_set_name(tree, 2, ":)");


  v_int* children = v_tree_get_all_children(tree, 0); // get list of children (not recursive currently)
  printf("Number of children of branch 0 (recursive): %d\n", children->len); // print number of children (2 in this case)
  v_int_free(children); // deallocate memory

  v_tree_print(tree);

  v_tree_free(tree); // Deallocate memory

  return 0;
}
