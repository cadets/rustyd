# Vagrant environment

This document gives a brief description of the Vagrant environment used for testing the *rustyd* client.

All of the virtual machines in the test setup are configured with two network interfaces. It is a hard and fast requirement of Vagrant requires the first interface is NAT'd. If this not present Vagrant is unable to configure port formwarding between the guest and host. As a result SSH access (`vagrant shh`) wouldn't work and provisioning the VM fails.

The virtual machines' second interface is configured for the network vboxnet0 (172.16.100.0/24).  DHCP is used to assign IP addresses on the vboxnet0 network. This can be achieved either with VirtualBox's internal DHCP server or with a stand alone DHCP server such as *dnsmasq*. (I prefer the latter option as it gives greater control and visibility). Note that the configuration file _pxeboot-dnsmasq.conf_ can be used to configure dnsmasq for the vboxnet0 network as follows:

`# dnsmasq --conf-file=pxeboot-dnsmasq.conf`

(Note: # indicates a command ran with root user permissions and $ a command ran as the user.)

## Commit log

The Vagrant group _commit\_log_ consists of a single virtual machine *kafka*.  The kafka virtual machine is based on a *bento/ubuntu-16.04* box. To start the virtual machine:

`$ vagrant up kafka`

To provision the virtual machine with Confluent platform (Apache Zookeeper and Kafka):

`$ vagrant provision kafka`

The Ansible provisioning script creates the folowing znodes in Zookeeper:

- /ddtrace
- /ddtrace/instrumentation
- /ddtrace/ep1 
...
- /ddtrace/epn (for each node in the group managed endpoints)

Provisioning aslo creates the Kafka topic *ddtrace-query-response*.

(Note: To provision the virtual machines the host machine musthave Ansible installed.)

## Managed enpoints

The Vagrant group _managed\_endpoints_ consists of two virtual machines *ep1* and *ep2*. The virtual machine ep1 is based on a *freebsd/FreeBSD-11.0-RC2* box. To start the virtual machine:

`$ vagrant up ep1`

To provision the virtual machine with the _rustyd_ client:

`$ vagrant provision ep1`

Provisioning the ep1 virtual machine performs a git clone of the rustyd source from: [https://github.com/cadets/rustyd](kafka)

This rustyd source is then automatically build and ran:

`# cargo run -- -z kafka.cadets:2181 -o ddtrace-query-response -b kafka.cadets:9092`

Where:

- -z specifies the Zookeeper sever
- -o is the Kafka topic to respond on
- -b specifies the Kafka server

(Note:the -b abd -o options are the process of being obsoleted. When completed all configuration of the rustyd client, except for the address of the Zookeeper server, will be performed through Zookeeper.)

### PXE booting

The virtual machine *ep2* is based on the *FreeBSD_cadets* box (found in the repository). This box (created using HashiCorp's Packer tool) allows PXE booting. The configuration for PXE booting is given in the pxeboot-dnsmasq.conf file.

By default the root-path for PXE booting is given as: `128.232.64.163:/exports/users/gcj21/freebsd-root` (where 128.232.64.163 is the IP address of the machine vica.cl.cam.ac.uk). The pxeboot file is served using tftp from the location: `/var/lib/tftpboot`. Both these values can be freely changed according to the desired setup.

## OPUS intergation

The section briefly outlines to integrate the Vagrant rustyd environment with [OPUS](ihttps://www.cl.cam.ac.uk/research/dtg/fresco/opus).

### Provision the OPUS virtual machine

The *opus* virtual machine is based on *bento/ubuntu-16.04* box. To start the virtual machine:

`$ vagrant up opus`

To provision the virtual machine with OPUS:

`vagrant provision opus`

The Ansible provisioning script (_roles/opus\_backend/tasks/main.yml_) will perform a git clone of the sources from the branch *cadets_rusty* from the repo *https://github.com/cadets/opus.git*.

To setup the OPUS server first ssh to the virtual machine (`vagrant ssh opus`). Then run `opusctl conf` (note the opus distribution is found in /home/vagrant/opus/dist).  When configuring set the address for provenance data collection to (substituting 54254 for the desired port number):

`tcp://localhost:54254`

For instructions on starting and stopping the OPUS server see: [https://github.com/cadets/opus](https://githib.com/cadets/opus). 

### rustyd client 

The rustyd client must be configured to use the TCP plugable transport (libddtrace_tcp.so). For compatibility with OPUS the managed endpoints must be instrumented with the [_audit.d_](https://raw.githubusercontent.com/cadets/dtrace-scripts/master/audit.d) script.

To instrument the endpoint the audit.d script must first be prepocessed with the C preprocessor:

`$ gcc -E -o audit-preprocessed.d-< dtrace-scripts/audit.d`

Then from a machine with zookeepercli installed, create a Zookeeper znode with the script as data:

`$ zookeepercli -servers kafka.cadets:2181 -c create /dtrace/instrumentation/audit.d "$(<audit-preprocessed.d)"`

To remove the instrumentation, the Zookeeper znode can be removed as follows:

`$ zookeepercli -servers kafka.cadets:2181 -c rm /dtrace/instrumentation/audit.d`

### Limitations

- Currenntly configuration of the rustyd client for using the TCP transport is sadly lacking. 
- The audit.d DTrace script generates spurious \0 character sequences. These are stripped out by OPUS at some performance cost (as each record is copied).
- Error handling in the rustyd client and OPUS appears to need some work.

