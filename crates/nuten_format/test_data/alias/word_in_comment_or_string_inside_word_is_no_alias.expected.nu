{
    a: {|| # x alias catch = save
        catch [a, b]
    }
}
{ b: {|| print "x alias trap = save"; trap [a,  b] } }
