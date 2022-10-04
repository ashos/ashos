#include<stdio.h>
#include<stdlib.h>
#include "cmd.h"

int main(void) {
  v_str* files = listdir(".",false);
  v_str* dirs = listdir(".",true);

  char* join_files = v_str_join(files, ';');
  char* join_dirs = v_str_join(dirs, ';');

  printf("%s \n", join_files);
  printf("\n%s \n", join_dirs);

  free(join_files);
  free(join_dirs);
  v_str_free(dirs);
  v_str_free(files);
}
