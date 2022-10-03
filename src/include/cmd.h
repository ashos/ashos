#ifndef __CMD_H_
#define __CMD_H_
#define _GNU_SOURCE // for asprintf
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "vectors.h"


#define CMD_LINE_BUFSIZE 256 // by default 256 chars for line in command output
#define CMD_BUFSIZE CMD_LINE_BUFSIZE * 64 // and 64 lines

#define GET_CMD_OUTPUT(cmd, excode) cmd_with_output((size_t)CMD_LINE_BUFSIZE, (size_t)CMD_BUFSIZE, cmd, excode);

string cmd_with_output(size_t line_len, size_t len, char* cmd, int* excode) {
  *excode = 0;
  FILE* fp;
  fp = popen(cmd, "r");

  if (fp == NULL)
    *excode = 1;

  char* out = malloc(sizeof(char) * len);
  char* line = malloc(sizeof(char) * line_len);

  int i = 0;
  while (fgets(line, line_len, fp) != NULL) {
    if (++i * line_len > len) {
      break;
    }
    strncat(out, line, line_len);
  }

  out[len - 1] = '\0';

  free(line);
  pclose(fp);
  string s;
  s.len = len;
  s.str = out;
  return s;
}

#endif
