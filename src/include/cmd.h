/*
AshOS Command interfacing library
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
#ifndef __CMD_H_
#define __CMD_H_
//#define _GNU_SOURCE // for asprintf
#include <stdlib.h>
#include <limits.h>       //For PATH_MAX
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <stdbool.h>
#include <sys/types.h>
#include <dirent.h>
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

v_str* listdir(const char *name, int indent, bool only_dirs) {
  v_str* files = v_str_new(0);
  DIR *dir;
  struct dirent *entry;

  if (!(dir = opendir(name)))
    return files;

  while ((entry = readdir(dir)) != NULL) {
    if (entry->d_type == DT_DIR) {
      char path[1024];
      if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0)
        continue;
      snprintf(path, sizeof(path), "%s/%s", name, entry->d_name);
      if (only_dirs) {
        char buf[PATH_MAX + 1];
        realpath(entry->d_name, buf);
        v_str_push_string(files, buf);
      }
      listdir(path, indent + 2, only_dirs);
    } else {
      if (!only_dirs) {
        char buf[PATH_MAX + 1];
        realpath(entry->d_name, buf);
        v_str_push_string(files, buf);
      }
      //printf("%*s- %s\n", indent, "", entry->d_name);
    }
  }
  closedir(dir);
  return files;
}

#endif
