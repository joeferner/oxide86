#!/bin/bash

grep 'CGA text' "${1:-oxide86.log}" | grep -oP "char='\\K[^']" | tr -d '\n'
