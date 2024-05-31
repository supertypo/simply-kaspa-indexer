# kaspa-db-filler-ng

**Not recommended for general use yet.**  

A high performance Kaspa DB Filler implemented in Rust.  

The filler has been implemented from scratch by deriving the functional spec of [kaspa-db-filler](https://github.com/lAmeR1/kaspa-db-filler).  
As part of this process the database schema was reworked to better support concurrency.  
This means that databases populated by the lAmeR1/kaspa-db-filler must be migrated to be compatible.  
A schema migration script has been developed and will be made available shortly.  
Compatible versions of api servers are in development/testing.

# License
Restricted, not for redistribution or use on other cryptocurrency networks. See LICENSE.
