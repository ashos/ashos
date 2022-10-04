/*
AshOS Vectors and string library
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
#ifndef __VECTORS_H_
#define __VECTORS_H_

#include<stdlib.h>
#include<string.h>

typedef struct v_int {
  int* data;
  size_t len;
} v_int;

typedef struct string {
  char* str;
  size_t len; // does not include NULL terminator - "hello" is length 5
} string;

typedef struct v_str {
  string* data;
  size_t len;
} v_str;


v_int* v_int_new(size_t); // create new n-size vector - v_int* vec = v_int_new(n);
void v_int_push(v_int*, int);
void v_int_pop(v_int*);
void v_int_free(v_int*); // dealloc allocated with v_int_new()
void v_int_reverse(v_int*); // reverse vector
void v_int_cat(v_int*, v_int*); // append second vector to first vector
void v_int_erase(v_int* vec, size_t pos, int n);


void str_set(string*, const char*); // str_set(&str, "string")
void str_copy(string*, string*);
void str_free(string*);


v_str* v_str_new(size_t);
void v_str_push(v_str*, string);
void v_str_push_string(v_str*, char*); // push regular char* string
char* v_str_join(v_str*, char space); // join all strings in vector together, separated by 'char space'
void v_str_pop(v_str*);
void v_str_cat(v_str*, v_str*);
void v_str_erase(v_str* vec, size_t pos, int n);
void v_str_free(v_str*);


#endif
