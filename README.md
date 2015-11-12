# ailmedak
ailmedak is kademlia spelled backwards!

Ailmedak is a collection of libraries and protocols to spin up massively available distributed hash tables. It is inspired by Kademlia proposed by Petar Maymounkov and  David Mazières.

Included is a binary to start a DHT node within a 160-bit namespace and a default k-factor of 50. Both of these can be tuneable (though the namespace size must be specified at compile time)
Ailmedak's long term goal is to be almost exclusively stack-based. While this may be useful for extremely stringent performance requirements (but probably not) and/or in embedded systems the true reason behind this is because forcing no dynamic memory is a fun challenge :)
