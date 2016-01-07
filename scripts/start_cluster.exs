#note: elixir is not the best fit for this when running locally as the erlang's :os.cmd
#cannot return the pids of spawned processes. python/perl is a better choice...
#or even bash. but this can spawn things pretty quickly and elixir is cool
#so it will suffice for now. also only works for unix
#in the future, I anticipate that OTP will serve useful for distributed deployments

#usage:
# $ elixir scripts/start_cluster.exs --exec={/path/to/ailmedak/executable} --n={number of nodes}

defmodule StartCluster do
  @port_start 3000
  def start do
    {exec, n} = StartCluster.get_opts
    (0..n
     |> Enum.map(&(RunLocal.run(get_tup(&1), exec))))
    Process.exit(self(), :normal)
  end
  def get_tup s do
    something = s * 2 + @port_start
    {something, something + 1}
  end
  def get_opts do 
    {values, _, _} = OptionParser.parse(System.argv(), switches: [exec: :string, n: :integer])
    {values[:exec], values[:n]}
  end
end

defmodule RunLocal do
  def run(port_tup, exec) do
    {cluster_port, api_port} = port_tup
    IO.puts "cluster port: #{cluster_port}, api_port: #{api_port}"
    spawn fn ->
      :os.cmd('#{exec} -p #{cluster_port} -a #{api_port}')
    end
  end
end

StartCluster.start
