# LocalDev

## Prerequisites

* [Nginx](https://www.nginx.com/)
* [mkcert](https://github.com/FiloSottile/mkcert) 
* [dnsmasq](https://thekelleys.org.uk/dnsmasq/doc.html) (optional)

## About

This scripts manages the Nginx reverse proxies for local development.

By default it will try to locate the Nginx configuration in `/usr/local/etc/nginx/nginx.conf` or
`/etc/nginx/`. If your configuration is in another location use the argument `--nginx_path
/path/to/nginx.conf`.

The `nginx.conf` must also define an include directive like `include servers/*` where the script
will write the new configuration files. The current user must have write permissions in that directory.

Certificates are auto-generated and setup using [mkcert](https://github.com/FiloSottile/mkcert).

After each configuration change the Nginx server is automatically reloaded with `nginx -s reload`.

## DNS

Because `/etc/hosts` requires special privileges and each new server name would have to be entered,
it is recommended to setup `dnsmasq` or a similar service to resolve all the `.localdev` names to
127.0.0.1

Otherwise you will have to manually insert created hostnames into `/etc/hosts` like this:
```
sudo echo "127.0.0.1 my-app.localdev" >> /etc/hosts
```

### Simple setup

In `/usr/local/etc/dnsmasq.conf`:
```
address=/.localdev/127.0.0.1
listen-address=127.0.0.1
no-dhcp-interface=
```

In a created (as root) `/etc/resolver/localdev`:
```
nameserver 127.0.0.1
```

You will probably also have to setup the DNS server `127.0.0.1` at the top of the DNS list in the
Network Preferences of the connected interface.


## FAQ

Why not `.dev` ?
- because this domain is used https://domains.google/tld/dev/

Why not `.local` ?
- This is reserved and may conflict with Bonjour and have issues on macOS.


## Using Websockets for HMR

By default this sets up a proxy for `wss://my-app.localdev/ws` to `localhost:3000`.

To setup a different proxy use the `--ws` or `-w` flag during the `add` command, for example if the HMR
server runs on port 3000 so `ws://localhost:3000/ws` you can do:
```
$ localdev add test --ws ws:3000
```

Then the HMR must be configured to tell the client to use the proper URL.

For [Vite](https://vitejs.dev/config/#server-hmr), you have to set both the port and the path in
the `vite.config.js` (or the Vite section of `svelte.config.js`):
```javascript
  server: {
    hmr: {
      path: '/ws',
      clientPort: 443
    }
  }
```

Or you may use a different port for that application:
```javascript
  server: {
    port: 3010,
    hmr: {
      path: '/ws',
      clientPort: 443
    }
  }
```

And use:
```
$ localdev add my-vite-app -p :3010 -w ws:localhost:3010
```

For [React](https://create-react-app.dev/docs/advanced-configuration/) apps simply use an environment variable `WDS_SOCKET_PORT=443`, this goes in the
`package.json` on the start script:
```javascript
{
    ...
    "start": "WDS_SOCKET_PORT=443 react-scripts start",
    ...
}
```

## Usage


* `localdev` prints all the current configured servers and their proxies (with `.localdev` domains).
  For example:
  ```
  $ localdev

   🚦 https://test.localdev
     🚀 /      => http://localhost:3000
     🚀 /api   => http://localhost:8080
     🚀 /api2  => http://localhost:8081

  ```

* `localdev find my-app` prints the current configuration for the given server.
  For example:
  ```
  $ localdev find my-app

   🚦 https://test.localdev
     🚀 /      => http://localhost:3000
     🚀 /api   => http://localhost:8080
     🚀 /api2  => http://localhost:8081

  ```

* `localdev add my-app` creates a configuration for https://my-app.localdev that proxies all request to
http://localhost:3000
  For example:
  ```
  $ localdev add my-app

   🚦 https://my-app.localdev
     🚀 /  => http://localhost:3000
  ```
  Advanced options:
  * `-p` or `--proxy` to define a proxy endpoint:
    * `-p [endpoint]:[port]` for proxying ``/endpoint`` to `http://localhost:port`
    * `-p :3000` equivalent to `/:3000` for most webapps running a dev server on port 3000
    * `-p api:8080` for proxying all requests to `/api` to `http://localhost:8080`
    * `-p api:8080/api` for proxying all requests to `/api` to `http://localhost:8080/api`
  * `-w` or `--ws` to define the websocket proxy (defaults to `/ws` -> `localhost:3000`)
  * `-o` or `--open` to immediately open the root URL in a browser
  * `--force` overwrite if the target configuration file already exists


  ```
  $ localdev add my-app :3001 -p api:8080 -p api2:8081/api -o

   🚦 https://my-app.localdev
     🚀 /     => http://localhost:3001
     🚀 /api  => http://localhost:8080
     🚀 /api2 => http://localhost:8081/api
  ```


* `localdev remove my-app` removes the configuration for https://my-app.localdev

* `localdev completion --shell` removes the configuration for https://my-app.localdev



