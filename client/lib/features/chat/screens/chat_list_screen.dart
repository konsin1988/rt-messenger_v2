import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

class ChatListScreen extends StatelessWidget {
  const ChatListScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Chats'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            onPressed: () {
              // TODO: Show create room dialog
            },
          ),
        ],
      ),
      body: ListView.builder(
        itemCount: 0, // TODO: Load from gRPC
        itemBuilder: (context, index) {
          return ListTile(
            leading: CircleAvatar(
              child: Text('U$index'),
            ),
            title: const Text('Room Name'),
            subtitle: const Text('Last message...'),
            onTap: () => context.push('/chats/$index'),
          );
        },
      ),
    );
  }
}
