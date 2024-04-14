while ! cqlsh -e 'describe cluster' ; do
    sleep 1
done