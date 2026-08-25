import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:grpc/grpc.dart';
import '../../../core/api/grpc_client.dart';
import '../../../core/api/generated/messenger.pb.dart';

const _storage = FlutterSecureStorage();
const _kTokenKey = 'jwt_token';
const _kUserIdKey = 'user_id';

class AuthState {
  final String? token;
  final User? user;
  final bool isLoading;
  final String? error;
  const AuthState({this.token, this.user, this.isLoading = false, this.error});
  bool get isAuthenticated => token != null && token!.isNotEmpty;
  AuthState copyWith({String? token, User? user, bool? isLoading, String? error}) =>
      AuthState(
        token: token ?? this.token,
        user: user ?? this.user,
        isLoading: isLoading ?? this.isLoading,
        error: error,
      );
}

class AuthNotifier extends AsyncNotifier<AuthState> {
  @override
  Future<AuthState> build() async {
    final token = await _storage.read(key: _kTokenKey);
    // Optionally validate via getUser, but just restore token for now
    return AuthState(token: token);
  }

  Future<void> _persist(String token, User? user) async {
    await _storage.write(key: _kTokenKey, value: token);
    if (user != null) {
      await _storage.write(key: _kUserIdKey, value: user.id);
    }
    state = AsyncData(AuthState(token: token, user: user));
  }

  CallOptions get callOptions {
    final token = state.value?.token;
    if (token == null || token.isEmpty) return CallOptions();
    return CallOptions(metadata: {'authorization': 'Bearer $token'});
  }

  Future<String?> requestOtp(String phone) async {
    state = const AsyncLoading();
    try {
      final client = ref.read(grpcClientProvider);
      final resp = await client.authService.requestOTP(
        RequestOTPRequest(phone: phone),
      );
      state = AsyncData(state.value ?? const AuthState());
      return resp.debugOtp.isNotEmpty ? resp.debugOtp : null;
    } on GrpcError catch (e) {
      state = AsyncData(AuthState(error: e.message ?? e.codeName));
      rethrow;
    } catch (e) {
      state = AsyncData(AuthState(error: e.toString()));
      rethrow;
    }
  }

  Future<void> verifyOtp(String phone, String code, {String username = ''}) async {
    state = const AsyncLoading();
    try {
      final client = ref.read(grpcClientProvider);
      final resp = await client.authService.verifyOTP(
        VerifyOTPRequest(phone: phone, code: code, username: username),
      );
      await _persist(resp.token, resp.user);
      state = AsyncData(AuthState(token: resp.token, user: resp.user));
    } on GrpcError catch (e) {
      state = AsyncData(AuthState(error: e.message ?? e.codeName));
      rethrow;
    }
  }

  Future<void> refreshToken() async {
    final token = state.value?.token;
    if (token == null) return;
    try {
      final client = ref.read(grpcClientProvider);
      final resp = await client.authService.refreshToken(
        RefreshTokenRequest(token: token),
      );
      await _persist(resp.token, resp.user);
    } catch (_) {}
  }

  Future<void> logout() async {
    await _storage.delete(key: _kTokenKey);
    await _storage.delete(key: _kUserIdKey);
    state = const AsyncData(AuthState(token: null, user: null));
  }

  Future<String?> getToken() async {
    if (state.value?.token != null) return state.value!.token;
    final t = await _storage.read(key: _kTokenKey);
    return t;
  }
}

final authProvider = AsyncNotifierProvider<AuthNotifier, AuthState>(AuthNotifier.new);

final authTokenProvider = Provider<String?>((ref) {
  return ref.watch(authProvider).value?.token;
});
