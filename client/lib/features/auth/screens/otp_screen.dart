import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../providers/auth_provider.dart';

class OtpScreen extends ConsumerStatefulWidget {
  final String phone;
  const OtpScreen({super.key, required this.phone});

  @override
  ConsumerState<OtpScreen> createState() => _OtpScreenState();
}

class _OtpScreenState extends ConsumerState<OtpScreen> {
  final _codeController = TextEditingController();
  final _usernameController = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  bool _isLoading = false;
  int _resendSeconds = 60;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _startTimer();
  }

  @override
  void dispose() {
    _codeController.dispose();
    _usernameController.dispose();
    _timer?.cancel();
    super.dispose();
  }

  void _startTimer() {
    _resendSeconds = 60;
    _timer?.cancel();
    _timer = Timer.periodic(const Duration(seconds: 1), (t) {
      if (_resendSeconds <= 1) {
        t.cancel();
        setState(() => _resendSeconds = 0);
      } else {
        setState(() => _resendSeconds--);
      }
    });
  }

  Future<void> _verify() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() => _isLoading = true);
    try {
      await ref.read(authProvider.notifier).verifyOtp(
            widget.phone,
            _codeController.text.trim(),
            username: _usernameController.text.trim(),
          );
      if (!mounted) return;
      context.go('/chats');
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Verify failed: $e')));
    } finally {
      if (mounted) setState(() => _isLoading = false);
    }
  }

  Future<void> _resend() async {
    if (_resendSeconds > 0) return;
    try {
      final debugOtp = await ref.read(authProvider.notifier).requestOtp(widget.phone);
      if (!mounted) return;
      if (debugOtp != null) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Debug OTP: $debugOtp')));
      } else {
        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Code resent')));
      }
      _startTimer();
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Resend failed: $e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(widget.phone)),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Form(
            key: _formKey,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text('Enter 6-digit code', style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                Text('Sent to ${widget.phone}. Mock code shown as SnackBar when SMS_MOCK=true.', style: Theme.of(context).textTheme.bodySmall),
                const SizedBox(height: 24),
                TextFormField(
                  controller: _codeController,
                  keyboardType: TextInputType.number,
                  maxLength: 6,
                  decoration: const InputDecoration(hintText: '123456', prefixIcon: Icon(Icons.lock_outline), counterText: ''),
                  validator: (v) {
                    if (v == null || v.isEmpty) return 'Enter code';
                    if (v.length != 6 || int.tryParse(v) == null) return '6 digits';
                    return null;
                  },
                ),
                const SizedBox(height: 16),
                TextFormField(
                  controller: _usernameController,
                  decoration: const InputDecoration(
                    hintText: 'Username (optional, for first registration)',
                    prefixIcon: Icon(Icons.person_outline),
                  ),
                  validator: (v) {
                    if (v == null || v.isEmpty) return null;
                    final re = RegExp(r'^[a-zA-Z0-9_-]{3,32}$');
                    if (!re.hasMatch(v)) return '3-32 alphanum _-';
                    return null;
                  },
                ),
                const SizedBox(height: 24),
                ElevatedButton(
                  onPressed: _isLoading ? null : _verify,
                  child: _isLoading
                      ? const SizedBox(height: 20, width: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Verify & Login'),
                ),
                const SizedBox(height: 16),
                TextButton(
                  onPressed: _resendSeconds == 0 ? _resend : null,
                  child: Text(_resendSeconds == 0 ? 'Resend code' : 'Resend in $_resendSeconds s'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
