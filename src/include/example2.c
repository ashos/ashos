#include <stdlib.h>
#include <stdio.h>
#include "vectors.h"
#include "cmd.h"

int main(void) {
  string str; // create new string
  int excode; // store exit code
  str = GET_CMD_OUTPUT("ls /", &excode); // get output of command, along with exit code
  printf("%s", str.str); // print out string output
  str_free(&str); // deallocate memory
}
