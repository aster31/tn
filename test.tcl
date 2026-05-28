#!/usr/bin/env tclsh

load target/debug/libtn.so

tn::pretty [tn::array [list]]
tn::pretty [tn::array {1 2}]
tn::pretty [tn::array {{1 2} {3 4}}]
tn::pretty [tn::array {{1 2 3} {-45 2 3} {5.3 2 3}}]
