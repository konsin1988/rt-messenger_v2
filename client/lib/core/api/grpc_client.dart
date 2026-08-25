import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:grpc/grpc.dart';
import 'generated/messenger.pbgrpc.dart';

const _serverHost = 'localhost';
const _serverPort = 50051;

final grpcClientProvider = Provider<GrpcClient>((ref) {
  return GrpcClient(host: _serverHost, port: _serverPort);
});

class GrpcClient {
  final String host;
  final int port;
  late final ClientChannel _channel;
  late final UserServiceClient userService;
  late final MessageServiceClient messageService;
  late final ChatRoomServiceClient chatRoomService;
  late final AuthServiceClient authService;

  GrpcClient({required this.host, required this.port}) {
    _channel = ClientChannel(
      host,
      port: port,
      options: const ChannelOptions(
        credentials: ChannelCredentials.insecure(),
      ),
    );

    userService = UserServiceClient(_channel);
    messageService = MessageServiceClient(_channel);
    chatRoomService = ChatRoomServiceClient(_channel);
    authService = AuthServiceClient(_channel);
  }

  CallOptions authOptions(String token) =>
      CallOptions(metadata: {'authorization': 'Bearer $token'});

  Future<void> dispose() async {
    await _channel.shutdown();
  }
}
