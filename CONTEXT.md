# NetMap

NetMap shows this PC's observed connections and their approximate geographic
locations. This glossary names the connection facts and what the map represents.

## Language

**Process instance**:
One running occurrence of a program. A PID may refer to different process
instances at different times.

**Endpoint**:
A network address and port, including an IPv6 scope where applicable. Local and
remote identify its relationship to this PC.

**TCP connection**:
One TCP lifetime between local and remote endpoints. A later connection can
reuse the same endpoints in the same process without being the same connection.

**Observed UDP flow**:
Datagrams observed between one process's local and remote endpoints. It does not
establish the lifetime of a UDP socket or imply a TCP-style connection.

**Observation**:
The facts NetMap has captured about a TCP connection or an observed UDP flow,
including available activity and location information. An observation can
describe a recently ended connection as well as a current one.

**Establishment highlight**:
The initial bright appearance of a newly observed connection, followed by its
configured dimming period. It denotes establishment, not ongoing traffic.

**Activity**:
Measured send/receive bytes in the current observation interval. An open TCP
connection may be idle, and an establishment highlight does not imply activity.

**Traffic pulse**:
A directional indication of measured activity on a guide: send toward the
destination and receive toward the origin.

**Place**:
A geographic scope: a country, a supplied coordinate, or a country's
observations that have no supplied coordinate.

**Group**:
The observations represented by one place, with their combined counts and
activity.

**Destination**:
A map representation of a group at a supplied or estimated position. Several
estimated destinations can represent the same group; they do not assign its
individual connections to cities.

**Guide**:
A geographic great-circle arc from the origin to a destination. It describes
endpoint locations, not a measured network path.
_Avoid_: Network route

**Origin**:
The geographic reference used as the starting point for the map's guides. It is
an overview location, not a measured starting coordinate for every connection.
