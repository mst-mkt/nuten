{a:{|| alias catch = save; catch [a,  b]}}
print {a:(do { alias trap = save; trap [a,  b] })}
print {a:(do { alias snag = save
  snag [a,  b] }), b: 1}
