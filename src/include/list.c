#include<stdio.h>
#include<stdlib.h>
#include "cmd.h"

int main(void) {
  v_str* files = listdir(".",0,false);
  v_str* dirs = listdir(".",0,true);

  char* join_files = v_str_join(files, ';');
  char* join_dirs = v_str_join(dirs, ';');

  printf("%s \n", join_files);
  printf("\n%s \n", join_dirs);

  free(join_files);
  free(join_dirs);
  free(dirs);
  free(files);
}
